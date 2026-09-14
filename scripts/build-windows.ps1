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
    [switch]$SkipInstaller
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
    dotnet publish (Join-Path $root "windows/src/DmxMoney.App/DmxMoney.App.csproj") `
        -c $Configuration -r $Rid --self-contained true `
        -p:Version=$Version -p:PublishReadyToRun=true `
        -o $publish

    if ($SkipInstaller) {
        Write-Host "==> Terminé : $publish"
        return
    }

    if (-not (Get-Command vpk -ErrorAction SilentlyContinue)) {
        Write-Host "==> Velopack absent : dotnet tool install -g vpk"
        return
    }

    Write-Host "==> Installeur Velopack"
    vpk pack --packId DmxMoney --packTitle DmxMoney --packVersion $Version `
        --packDir $publish --mainExe DmxMoney.exe `
        --icon (Join-Path $root "windows/src/DmxMoney.App/Assets/dmxmoney.ico") `
        --outputDir (Join-Path $root "target/windows/releases/$Rid")
}
finally {
    Pop-Location
}
