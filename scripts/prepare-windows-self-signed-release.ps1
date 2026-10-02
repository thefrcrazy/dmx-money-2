<# Importe une identité autosignée persistante, sans confiance publique ni export de clé. #>
param(
    [Parameter(Mandatory)][string]$PfxPath,
    [Parameter(Mandatory)][Security.SecureString]$PfxPassword,
    [Parameter(Mandatory)][string]$ExpectedThumbprint,
    [Parameter(Mandatory)][string]$PublicCertificatePath
)
$ErrorActionPreference = 'Stop'
if ($ExpectedThumbprint -notmatch '\A[0-9a-fA-F]{40}\z') {
    throw 'ExpectedThumbprint invalide : 40 caractères hexadécimaux requis.'
}
if ($PfxPassword.Length -eq 0) { throw 'Un mot de passe PFX non vide est requis.' }
if (-not $IsWindows) { throw "L'import du certificat de publication nécessite Windows." }
$PublicCertificatePath = [IO.Path]::GetFullPath($PublicCertificatePath)
if (Test-Path -LiteralPath $PublicCertificatePath) { throw 'Le certificat public existe déjà.' }
$pfxBytes = [IO.File]::ReadAllBytes((Resolve-Path -LiteralPath $PfxPath).Path)
$thumbprint = $null
try {
    if (-not ('DmxMoney.SelfSignedPfx' -as [type])) {
        Add-Type -Path (Join-Path $PSScriptRoot 'windows/SelfSignedPfx.cs')
    }
    $thumbprint = [DmxMoney.SelfSignedPfx]::Import($pfxBytes, $PfxPassword, $ExpectedThumbprint)
    . (Join-Path $PSScriptRoot 'windows-development-signing.ps1')
    $certificate = Get-DevelopmentCodeSigningCertificate -Thumbprint $thumbprint -RequirePrivateKey
    $null = New-Item -ItemType Directory -Force -Path (Split-Path -Parent $PublicCertificatePath)
    $null = Export-Certificate -Cert $certificate -FilePath $PublicCertificatePath -Type CERT
    Write-Warning 'Publication autosignée : identité locale non validée par une autorité publique ; aucune garantie Smart App Control.'
    [pscustomobject]@{ Thumbprint = $thumbprint; PublicCertificatePath = $PublicCertificatePath; Subject = $certificate.Subject }
}
catch {
    if ($thumbprint) { Remove-Item -LiteralPath "Cert:/CurrentUser/My/$thumbprint" -DeleteKey -Force }
    throw
}
finally { [Array]::Clear($pfxBytes, 0, $pfxBytes.Length) }
