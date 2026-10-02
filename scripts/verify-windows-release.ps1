<# Vérifie ce qui est distribué : installeur, payload complet et empreintes du flux Velopack. #>
[CmdletBinding(DefaultParameterSetName = 'PublicTrust')]
param(
    [Parameter(Mandatory)][string]$ReleaseDirectory,
    [Parameter(Mandatory)][ValidateSet("win-x64", "win-arm64")][string]$Rid,
    [Parameter(Mandatory, ParameterSetName = 'PublicTrust')]
    [Parameter(ParameterSetName = 'Development')][string]$SignToolPath,
    [Parameter(Mandatory, ParameterSetName = 'Development')][string]$DevelopmentCertificateThumbprint
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true
$ReleaseDirectory = (Resolve-Path -LiteralPath $ReleaseDirectory).Path
$developmentCertificate = $null
$script:developmentSignatureCount = 0
$script:publicSignatureCount = 0
if ($PSCmdlet.ParameterSetName -eq 'Development') {
    . (Join-Path $PSScriptRoot 'windows-development-signing.ps1')
    $developmentCertificate = Get-DevelopmentCodeSigningCertificate -Thumbprint $DevelopmentCertificateThumbprint
    $SignToolPath = Get-WindowsSignTool -Path $SignToolPath
}
else { $SignToolPath = (Resolve-Path -LiteralPath $SignToolPath).Path }
$setup = Join-Path $ReleaseDirectory "DmxMoney-$Rid-Setup.exe"
$feedPath = Join-Path $ReleaseDirectory "releases.$Rid.json"
if (-not (Test-Path -LiteralPath $setup -PathType Leaf)) { throw "Installeur $Rid absent." }

function Assert-TrustedSignature([string]$Path, [bool]$RequireDevelopmentPin = $false) {
    if ($developmentCertificate) {
        try {
            Assert-DevelopmentAuthenticode -Path $Path -Certificate $developmentCertificate
            $script:developmentSignatureCount++
            return
        }
        catch {
            if ($RequireDevelopmentPin) { throw "Signature de développement obligatoire pour $Path : $($_.Exception.Message)" }
            # Velopack conserve les signatures fournisseurs déjà approuvées. Un autre
            # certificat n'est accepté qu'après vérification complète de confiance publique.
        }
        & $SignToolPath verify /pa /all /tw $Path
        $script:publicSignatureCount++
        return
    }
    # /pa applique la politique Authenticode ; /all vérifie les signatures ; /tw exige un timestamp.
    # SignTool renvoie 2 pour un avertissement : lui aussi fait échouer la release.
    & $SignToolPath verify /pa /all /tw $Path
}

function Assert-PayloadMachine([string]$Path, [uint16]$ExpectedMachine) {
    $stream = [IO.File]::OpenRead($Path)
    $reader = [IO.BinaryReader]::new($stream)
    try {
        if ($stream.Length -lt 64 -or $reader.ReadUInt16() -ne 0x5a4d) { throw "En-tête PE invalide : $Path" }
        $stream.Position = 0x3c
        $offset = $reader.ReadUInt32()
        if ($offset -gt $stream.Length - 6) { throw "En-tête PE invalide : $Path" }
        $stream.Position = $offset
        if ($reader.ReadUInt32() -ne 0x4550 -or $reader.ReadUInt16() -ne $ExpectedMachine) {
            throw "Architecture PE incorrecte pour $Rid : $Path"
        }
    }
    finally { $reader.Dispose(); $stream.Dispose() }
}

Assert-TrustedSignature $setup $true
$feed = Get-Content -LiteralPath $feedPath -Raw | ConvertFrom-Json
$fullPackages = @($feed.Assets | Where-Object { $_.Type -eq "Full" })
if ($fullPackages.Count -ne 1) { throw "Le flux $Rid doit contenir exactement un paquet complet." }
foreach ($asset in $feed.Assets) {
    if (-not $asset.FileName -or [System.IO.Path]::GetFileName($asset.FileName) -ne $asset.FileName -or
        $asset.FileName -match '[/\\:]' -or $asset.FileName -in @(".", "..")) {
        throw "Nom de paquet invalide dans le flux $Rid."
    }
    if ($asset.SHA256 -isnot [string] -or $asset.SHA256 -notmatch '\A[0-9a-fA-F]{64}\z') {
        throw "Empreinte SHA-256 invalide dans le flux $Rid : 64 caractères hexadécimaux sont requis."
    }
    $packagePath = Join-Path $ReleaseDirectory $asset.FileName
    $stream = [System.IO.File]::OpenRead($packagePath)
    try {
        # Velopack 0.0.1298 calcule un HEX de 64 caractères ; son ancien commentaire XML
        # indique base64, mais CalculateStreamSHA256 et le vrai flux utilisent bien HEX.
        $digest = [Convert]::ToHexString([System.Security.Cryptography.SHA256]::HashData($stream))
        if ($stream.Length -ne $asset.Size -or -not [StringComparer]::OrdinalIgnoreCase.Equals($digest, $asset.SHA256)) {
            throw "Taille ou empreinte SHA-256 incorrecte : $($asset.FileName)."
        }
    }
    finally {
        $stream.Dispose()
    }
}

$extraction = Join-Path ([System.IO.Path]::GetTempPath()) ([guid]::NewGuid().ToString("N"))
try {
    [System.IO.Compression.ZipFile]::ExtractToDirectory((Join-Path $ReleaseDirectory $fullPackages[0].FileName), $extraction)
    $manifestPath = Join-Path $extraction 'DmxMoney.nuspec'
    [xml]$manifest = Get-Content -LiteralPath $manifestPath -Raw
    $manifestRid = $manifest.SelectSingleNode("/*[local-name()='package']/*[local-name()='metadata']/*[local-name()='rid']")
    $manifestArchitecture = $manifest.SelectSingleNode("/*[local-name()='package']/*[local-name()='metadata']/*[local-name()='machineArchitecture']")
    $expectedArchitecture = if ($Rid -eq 'win-arm64') { 'arm64' } else { 'x64' }
    if (-not $manifestRid -or $manifestRid.InnerText -ne $Rid -or
        -not $manifestArchitecture -or $manifestArchitecture.InnerText -ne $expectedArchitecture) {
        throw "Architecture du manifeste incorrecte : $Rid attendu."
    }
    $expectedMachine = if ($Rid -eq 'win-arm64') { 0xaa64 } else { 0x8664 }
    Assert-PayloadMachine (Join-Path $extraction 'lib/app/DmxMoney.exe') $expectedMachine
    $binaries = @(Get-ChildItem -LiteralPath $extraction -File -Recurse |
        Where-Object { $_.Extension -in @(".exe", ".dll") })
    if (-not ($binaries | Where-Object Name -eq "DmxMoney.exe") -or
        -not ($binaries | Where-Object Name -eq "dmx_ffi.dll") -or
        -not ($binaries | Where-Object Name -eq "Squirrel.exe")) {
        throw "Paquet incomplet : application, noyau ou moteur de mise à jour absent."
    }
    foreach ($nativeCore in $binaries | Where-Object Name -eq 'dmx_ffi.dll') {
        Assert-PayloadMachine $nativeCore.FullName $expectedMachine
    }
    foreach ($binary in $binaries) {
        $ours = $binary.Name -match '\A(?:DmxMoney.*\.(?:exe|dll)|dmx_ffi\.dll)\z'
        Assert-TrustedSignature $binary.FullName $ours
    }
    if ($developmentCertificate) {
        Write-Host "==> $Rid : Setup + $($binaries.Count) binaires vérifiés ; $script:developmentSignatureCount signatures de développement épinglées (Setup inclus), $script:publicSignatureCount signatures fournisseurs publiquement approuvées ; SHA-256 HEX valide."
        Write-Warning 'Ce contrôle local ne prouve aucune confiance publique et ne garantit pas une autorisation Smart App Control.'
    }
    else { Write-Host "==> $Rid : installeur et $($binaries.Count) binaires signés, horodatés et vérifiés ; SHA-256 valide." }
}
finally {
    if (Test-Path -LiteralPath $extraction) { Remove-Item -LiteralPath $extraction -Recurse -Force }
}
