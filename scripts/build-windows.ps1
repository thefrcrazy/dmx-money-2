<#
.SYNOPSIS
    Construit DmxMoney pour Windows : noyau Rust, bindings C#, app WinUI, installeur Velopack.
.EXAMPLE
    pwsh scripts/build-windows.ps1 -Rid win-x64
#>
param(
    [string]$Configuration = "Release",
    [ValidateSet("win-x64", "win-arm64")][string]$Rid = "win-x64",
    [string]$Version = "",
    [switch]$SkipInstaller,
    [string]$SignToolPath = "",
    [string]$SigningDlibPath = "",
    [string]$SigningMetadataPath = "",
    [string]$DevelopmentCertificateThumbprint = "",
    [switch]$RequireSigning
)

$ErrorActionPreference = "Stop"
# Sans cette préférence, un code de sortie non nul de cargo ou dotnet n'interrompt pas le script :
# la CI affichait « Terminé » juste après un échec de compilation.
$PSNativeCommandUseErrorActionPreference = $true
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    if (-not $Version) {
        $Version = (Get-Content (Join-Path $root "VERSION")).Trim()
    }
    if ($Version -notmatch '^2\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z]+(?:[.-][0-9A-Za-z]+)*)?$') {
        throw "Version invalide : une version 2.x.y éventuellement suivie d'une pré-version est requise."
    }
    if ($RequireSigning -and $SkipInstaller) {
        throw "RequireSigning exige la construction et la vérification de l'installeur."
    }
    . (Join-Path $PSScriptRoot 'windows-development-signing.ps1')
    Assert-DevelopmentSigningOptions -Thumbprint $DevelopmentCertificateThumbprint -RequireSigning ([bool]$RequireSigning) `
        -SkipInstaller ([bool]$SkipInstaller) -SigningDlibPath $SigningDlibPath -SigningMetadataPath $SigningMetadataPath
    $developmentSigning = [bool]$DevelopmentCertificateThumbprint
    $signingConfigured = -not $developmentSigning -and ($SignToolPath -or $SigningDlibPath -or $SigningMetadataPath)
    if ($developmentSigning) {
        $DevelopmentCertificateThumbprint = $DevelopmentCertificateThumbprint.ToUpperInvariant()
        $null = Get-DevelopmentCodeSigningCertificate -Thumbprint $DevelopmentCertificateThumbprint -RequirePrivateKey
        $SignToolPath = Get-WindowsSignTool -Path $SignToolPath
        Write-Warning 'Signature autosignée de développement : aucune confiance publique ; Smart App Control peut bloquer ces fichiers.'
    }
    if ($RequireSigning -or $signingConfigured) {
        foreach ($path in @($SignToolPath, $SigningDlibPath, $SigningMetadataPath)) {
            if (-not $path -or -not (Test-Path -LiteralPath $path -PathType Leaf)) {
                throw "Signature Windows requise : SignToolPath, SigningDlibPath et SigningMetadataPath doivent exister."
            }
        }
        $SignToolPath = (Resolve-Path -LiteralPath $SignToolPath).Path
        $SigningDlibPath = (Resolve-Path -LiteralPath $SigningDlibPath).Path
        $SigningMetadataPath = (Resolve-Path -LiteralPath $SigningMetadataPath).Path
    }
    $target = if ($Rid -eq "win-arm64") { "aarch64-pc-windows-msvc" } else { "x86_64-pc-windows-msvc" }

    Write-Host "==> Noyau Rust ($target)"
    rustup target add $target | Out-Null
    cargo build -p dmx-ffi --release --target $target

    $nativeDirectory = Join-Path $root "windows/src/DmxMoney.Interop/runtimes/$Rid/native"
    New-Item -ItemType Directory -Force -Path $nativeDirectory | Out-Null
    Copy-Item (Join-Path $root "target/$target/release/dmx_ffi.dll") $nativeDirectory -Force

    if (Get-Command uniffi-bindgen-cs -ErrorAction SilentlyContinue) {
        Write-Host "==> Bindings C#"
        uniffi-bindgen-cs `
            --library (Join-Path $root "target/$target/release/dmx_ffi.dll") `
            --config (Join-Path $root "core/crates/dmx-ffi/uniffi.toml") `
            --out-dir (Join-Path $root "windows/src/DmxMoney.Interop/Generated")
    }
    else {
        Write-Host "==> Bindings C# : uniffi-bindgen-cs absent, on garde les fichiers générés"
    }

    Write-Host "==> Publication de l'application"
    $publish = Join-Path $root "target/windows/$Rid"
    # Évite qu'un ancien build laisse des DLL ou assets supprimés dans le nouveau paquet.
    if (Test-Path -LiteralPath $publish) {
        Remove-Item -LiteralPath $publish -Recurse -Force
    }
    $project = Join-Path $root "windows/src/DmxMoney.App/DmxMoney.App.csproj"
    # Sous dotnet, le Windows App SDK lance XamlCompiler.exe, qui échoue sans jamais afficher ses erreurs
    # (microsoft-ui-xaml#10027), et sa tâche .NET ne se charge pas avec MSBuild 18. Le MSBuild de
    # Visual Studio exécute le compilateur XAML en processus : c'est lui qu'on utilise quand il est là.
    $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio/Installer/vswhere.exe"
    $msbuild = if (Test-Path $vswhere) {
        & $vswhere -latest -products * -requires Microsoft.Component.MSBuild -find "MSBuild\**\Bin\amd64\MSBuild.exe" |
            Select-Object -First 1
    }
    if ($msbuild) {
        Write-Host "    MSBuild : $msbuild"
        $platform = if ($Rid -eq "win-arm64") { "ARM64" } else { "x64" }
        & $msbuild $project -restore -t:Publish -m -nologo -v:minimal `
            -p:Configuration=$Configuration -p:Platform=$platform -p:RuntimeIdentifier=$Rid `
            -p:SelfContained=true -p:PublishReadyToRun=true -p:Version=$Version "-p:PublishDir=$publish/"
    }
    else {
        dotnet publish $project -c $Configuration -r $Rid --self-contained true `
            -p:Version=$Version -p:PublishReadyToRun=true -o $publish
    }

    # WinUI charge le XAML compilé de l'app depuis resources.pri : sans ce fichier, l'application se
    # ferme dès son lancement, sans aucun message (voir EnableMsixTooling dans DmxMoney.App.csproj).
    if (-not (Test-Path (Join-Path $publish "resources.pri"))) {
        throw "resources.pri absent de $publish : l'application se fermerait au lancement."
    }

    if ($SkipInstaller) {
        Write-Host "==> Terminé : $publish"
        return
    }

    if (-not (Get-Command vpk -ErrorAction SilentlyContinue)) {
        throw "Velopack absent : dotnet tool install -g vpk --version 0.0.1298"
    }

    Write-Host "==> Installeur Velopack"
    # Un canal par architecture : les installeurs x64 et arm64 ont des noms distincts dans la
    # release, et l'app installée lit le flux de son canal (releases.<canal>.json).
    $releaseKind = if ($developmentSigning) { 'development-releases' } else { 'releases' }
    $releases = Join-Path $root "target/windows/$releaseKind/$Rid"
    if (Test-Path -LiteralPath $releases) {
        Remove-Item -LiteralPath $releases -Recurse -Force
    }
    $packArguments = @(
        "pack", "--packId", "DmxMoney", "--packTitle", "DmxMoney", "--packVersion", $Version,
        "--packDir", $publish, "--mainExe", "DmxMoney.exe", "--channel", $Rid,
        "--icon", (Join-Path $root "windows/src/DmxMoney.App/Assets/dmxmoney.ico"),
        "--outputDir", $releases
    )
    if ($developmentSigning) {
        # /sha1 sélectionne le certificat dans CurrentUser/My ; le digest du fichier reste SHA-256.
        $signTemplate = '"{0}" sign /s My /sha1 {1} /fd SHA256 /tr http://timestamp.acs.microsoft.com /td SHA256 {{{{file...}}}}' -f `
            $SignToolPath, $DevelopmentCertificateThumbprint
        $packArguments += @('--signTemplate', $signTemplate)
    }
    elseif ($signingConfigured) {
        # Velopack signe le payload ET les exécutables qu'il génère (Update, stub et Setup).
        # Sa version 0.0.1298 ne fournit pas le dlib : on utilise le SDK et le client Microsoft
        # explicitement installés par prepare-windows-signing.ps1, via le template officiel.
        $signTemplate = '"{0}" sign /fd SHA256 /tr http://timestamp.acs.microsoft.com /td SHA256 /dlib "{1}" /dmdf "{2}" {{{{file...}}}}' -f `
            $SignToolPath, $SigningDlibPath, $SigningMetadataPath
        $packArguments += @("--signTemplate", $signTemplate)
    }
    else {
        Write-Warning "Build local non signé : il peut être bloqué par Smart App Control. Ne pas le publier."
    }
    vpk @packArguments
    if ($developmentSigning) {
        & (Join-Path $PSScriptRoot 'verify-windows-release.ps1') -ReleaseDirectory $releases -Rid $Rid `
            -DevelopmentCertificateThumbprint $DevelopmentCertificateThumbprint
    }
    elseif ($signingConfigured) {
        & (Join-Path $PSScriptRoot "verify-windows-release.ps1") -ReleaseDirectory $releases -Rid $Rid -SignToolPath $SignToolPath
    }
}
finally {
    Pop-Location
}
