# Exécutable aussi sur macOS/Linux ; les vrais contrôles Authenticode restent sur Windows.
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$scripts = @("build-windows.ps1", "prepare-windows-signing.ps1", "verify-windows-release.ps1")
foreach ($script in $scripts) {
    $tokens = $null
    $parseErrors = $null
    $null = [System.Management.Automation.Language.Parser]::ParseFile(
        (Join-Path $root "scripts/$script"), [ref]$tokens, [ref]$parseErrors)
    if ($parseErrors.Count) { throw ($parseErrors | Out-String) }
}

function Assert-Rejected([scriptblock]$Action, [string]$Expected) {
    try { & $Action }
    catch {
        if ($_.Exception.Message -like "*$Expected*") { return }
        throw "Erreur inattendue : $($_.Exception.Message)"
    }
    throw "L'action dangereuse aurait dû être refusée : $Expected"
}

$build = Join-Path $root "scripts/build-windows.ps1"
Assert-Rejected { & $build -Version '2.0.7; unsafe' -SkipInstaller } "Version invalide"
Assert-Rejected { & $build -Version '2.0.7' -RequireSigning } "Signature Windows requise"
Assert-Rejected { & $build -Version '2.0.7' -RequireSigning -SkipInstaller } "RequireSigning exige"
Assert-Rejected { & $build -Version '2.0.7' -SigningMetadataPath '/missing/metadata.json' } "Signature Windows requise"
Assert-Rejected {
    & (Join-Path $root "scripts/prepare-windows-signing.ps1") -Endpoint 'http://weu.codesigning.azure.net' `
        -AccountName 'dmxmoney' -CertificateProfileName 'public-trust' -OutputDirectory ([System.IO.Path]::GetTempPath())
} "Endpoint Artifact Signing invalide"
Write-Host "PASS : scripts PowerShell valides ; version injectée, signature absente/incomplète et endpoint HTTP refusés avant build."
