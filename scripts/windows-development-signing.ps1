# Aides du mode de développement autosigné ; aucun magasin de confiance n'est modifié.
function Assert-DevelopmentSigningOptions {
    param([string]$Thumbprint, [bool]$RequireSigning, [bool]$SkipInstaller,
        [string]$SigningDlibPath, [string]$SigningMetadataPath,
        [bool]$SelfSignedRelease, [bool]$EphemeralSigningIdentity)
    if ($SelfSignedRelease -and -not $Thumbprint) {
        throw "SelfSignedRelease exige DevelopmentCertificateThumbprint : aucune publication non signée."
    }
    if ($EphemeralSigningIdentity -and -not $SelfSignedRelease) {
        throw "EphemeralSigningIdentity exige SelfSignedRelease."
    }
    if (-not $Thumbprint) { return }
    if ($Thumbprint -notmatch '\A[0-9a-fA-F]{40}\z') {
        throw "DevelopmentCertificateThumbprint invalide : 40 caractères hexadécimaux sont requis."
    }
    if ($RequireSigning -or $SigningDlibPath -or $SigningMetadataPath) {
        throw "La signature de développement est incompatible avec RequireSigning et Artifact Signing."
    }
    if ($SkipInstaller) { throw "La signature de développement exige un installeur à vérifier." }
}

function Get-WindowsSignTool {
    param([string]$Path)
    if (-not $IsWindows) { throw "La signature de développement nécessite Windows." }
    if (-not $Path) {
        $sdk = Join-Path ${env:ProgramFiles(x86)} "Windows Kits/10/bin"
        $Path = Get-ChildItem -LiteralPath $sdk -Directory | Where-Object { $_.Name -match '^10\.0\.\d+\.0$' } |
            Sort-Object { [version]$_.Name } -Descending |
            ForEach-Object { Join-Path $_.FullName 'x64/signtool.exe' } |
            Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } | Select-Object -First 1
    }
    if (-not $Path -or -not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "SignTool absent : installer le SDK Windows ou fournir SignToolPath."
    }
    return (Resolve-Path -LiteralPath $Path).Path
}

function Get-DevelopmentCodeSigningCertificate {
    param([string]$Thumbprint, [switch]$RequirePrivateKey)
    Assert-DevelopmentSigningOptions -Thumbprint $Thumbprint
    if (-not $IsWindows) { throw "La signature de développement nécessite Windows." }
    $certificate = Get-Item -LiteralPath "Cert:/CurrentUser/My/$Thumbprint" -ErrorAction Stop
    $now = Get-Date
    $eku = $certificate.Extensions | Where-Object { $_.Oid.Value -eq '2.5.29.37' }
    if ($certificate.Subject -cne $certificate.Issuer -or
        $now -lt $certificate.NotBefore -or $now -ge $certificate.NotAfter -or
        -not ($eku.EnhancedKeyUsages | Where-Object Value -eq '1.3.6.1.5.5.7.3.3')) {
        throw "Le certificat de développement doit être autosigné, valide et destiné à Code Signing."
    }
    $rsa = [System.Security.Cryptography.X509Certificates.RSACertificateExtensions]::GetRSAPublicKey($certificate)
    try {
        if (-not $rsa -or $rsa.KeySize -lt 3072) { throw "Le certificat de développement exige RSA 3072 bits minimum." }
    }
    finally { if ($rsa) { $rsa.Dispose() } }
    if ($RequirePrivateKey) {
        $privateKey = [System.Security.Cryptography.X509Certificates.RSACertificateExtensions]::GetRSAPrivateKey($certificate)
        try {
            if (-not $privateKey -or $privateKey -isnot [System.Security.Cryptography.RSACng] -or
                $privateKey.Key.ExportPolicy -ne [System.Security.Cryptography.CngExportPolicies]::None) {
                throw "La signature de développement exige une clé privée CNG non exportable dans CurrentUser/My."
            }
        }
        finally { if ($privateKey) { $privateKey.Dispose() } }
    }
    return $certificate
}

function Assert-DevelopmentAuthenticode {
    param([string]$Path, [System.Security.Cryptography.X509Certificates.X509Certificate2]$Certificate,
        [switch]$AllowMissingTimestamp)
    if (-not $IsWindows) { throw "La vérification Authenticode nécessite Windows." }
    if (-not ('DmxMoney.DevelopmentAuthenticode' -as [type])) {
        Add-Type -Path (Join-Path $PSScriptRoot 'windows/DevelopmentAuthenticode.cs')
    }
    # Get-AuthenticodeSignature peut privilégier un catalogue ; contrôle du CMS embarqué uniquement.
    [DmxMoney.DevelopmentAuthenticode]::VerifyFile((Resolve-Path -LiteralPath $Path).Path, $Certificate, -not $AllowMissingTimestamp)
}
