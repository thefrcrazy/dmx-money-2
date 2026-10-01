<# Crée une vraie clé locale de développement, sans l'ajouter aux racines de confiance. #>
param(
    [string]$PublicCertificatePath = "",
    [ValidateRange(1, 1095)][int]$ValidDays = 365
)
$ErrorActionPreference = 'Stop'
if (-not $IsWindows) { throw "La création du certificat de développement nécessite Windows." }
if (-not $PublicCertificatePath) {
    $PublicCertificatePath = Join-Path (Split-Path -Parent $PSScriptRoot) 'target/windows/development/Developmax-code-signing.cer'
}
$PublicCertificatePath = [IO.Path]::GetFullPath($PublicCertificatePath)
if (Test-Path -LiteralPath $PublicCertificatePath) { throw "Le certificat public existe déjà : $PublicCertificatePath" }
$null = New-Item -ItemType Directory -Force -Path (Split-Path -Parent $PublicCertificatePath)
$certificate = New-SelfSignedCertificate -Type CodeSigningCert -Subject 'CN=Collignon Maxim, O=Developmax' `
    -FriendlyName 'DmxMoney — Developmax / Collignon Maxim (développement local)' `
    -CertStoreLocation 'Cert:/CurrentUser/My' -KeyAlgorithm RSA -KeyLength 3072 -HashAlgorithm SHA256 `
    -Provider 'Microsoft Software Key Storage Provider' -KeyExportPolicy NonExportable `
    -NotAfter (Get-Date).AddDays($ValidDays)
try {
    . (Join-Path $PSScriptRoot 'windows-development-signing.ps1')
    $null = Get-DevelopmentCodeSigningCertificate -Thumbprint $certificate.Thumbprint -RequirePrivateKey
    # Export du certificat public seulement ; aucune clé privée/PFX et aucun import de confiance.
    $null = Export-Certificate -Cert $certificate -FilePath $PublicCertificatePath -Type CERT
}
catch {
    Remove-Item -LiteralPath "Cert:/CurrentUser/My/$($certificate.Thumbprint)" -DeleteKey -Force
    throw
}
Write-Warning 'Certificat autosigné de développement : aucune confiance publique et aucune garantie Smart App Control.'
[pscustomobject]@{ Thumbprint = $certificate.Thumbprint; PublicCertificatePath = $PublicCertificatePath; Subject = $certificate.Subject }
