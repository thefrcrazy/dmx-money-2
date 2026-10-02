# Notice publique testable partout ; vrai import CNG + signature réservés à Windows.
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
$scripts = Split-Path -Parent $PSScriptRoot
. (Join-Path $scripts 'windows-self-signed-release.ps1')
. (Join-Path $scripts 'windows-development-signing.ps1')
$temporary = Join-Path ([IO.Path]::GetTempPath()) ('dmx-self-signed-release-' + [guid]::NewGuid().ToString('N'))
$null = New-Item -ItemType Directory -Path $temporary
$rsa = [Security.Cryptography.RSA]::Create(3072)
$certificate = $null
$imported = $null
$fixturePassword = 'Fixture-only-' + [guid]::NewGuid().ToString('N')
$password = ConvertTo-SecureString $fixturePassword -AsPlainText -Force
function Assert-Rejected([scriptblock]$Action, [string]$Expected) {
    try { & $Action }
    catch {
        if ($_.Exception.Message -like "*$Expected*") { return }
        throw "Erreur inattendue : $($_.Exception.Message)"
    }
    throw "L'action aurait dû être refusée : $Expected"
}
try {
    $request = [Security.Cryptography.X509Certificates.CertificateRequest]::new('CN=Collignon Maxim, O=Developmax', $rsa,
        [Security.Cryptography.HashAlgorithmName]::SHA256, [Security.Cryptography.RSASignaturePadding]::Pkcs1)
    $ekus = [Security.Cryptography.OidCollection]::new()
    $null = $ekus.Add([Security.Cryptography.Oid]::new('1.3.6.1.5.5.7.3.3'))
    $request.CertificateExtensions.Add([Security.Cryptography.X509Certificates.X509EnhancedKeyUsageExtension]::new($ekus, $false))
    $request.CertificateExtensions.Add([Security.Cryptography.X509Certificates.X509KeyUsageExtension]::new(
        [Security.Cryptography.X509Certificates.X509KeyUsageFlags]::DigitalSignature, $true))
    # La notice portable n'a besoin que du certificat public ; ne crée aucune identité/keychain macOS.
    $generator = [Security.Cryptography.X509Certificates.X509SignatureGenerator]::CreateForRSA(
        $rsa, [Security.Cryptography.RSASignaturePadding]::Pkcs1)
    $certificate = $request.Create($request.SubjectName, $generator, (Get-Date).AddMinutes(-1),
        (Get-Date).AddDays(1), [Security.Cryptography.RandomNumberGenerator]::GetBytes(16))
    if ($IsWindows) {
        $withPrivateKey = [Security.Cryptography.X509Certificates.RSACertificateExtensions]::CopyWithPrivateKey($certificate, $rsa)
        $certificate.Dispose()
        $certificate = $withPrivateKey
    }
    foreach ($rid in @('win-x64', 'win-arm64')) {
        Write-WindowsSelfSignedReleaseNotice -ReleaseDirectory $temporary -Rid $rid -Version '2.0.7' -Certificate $certificate
        $metadata = Get-Content -LiteralPath (Join-Path $temporary "WINDOWS-SIGNATURE-$rid.json") -Raw | ConvertFrom-Json
        if ($metadata.signingMode -ne 'self-signed' -or $metadata.publiclyTrustedPublisher -or
            $metadata.smartAppControlCompatibilityGuaranteed -or $metadata.privateKeyIncluded -or $metadata.ephemeralSigningIdentity) {
            throw 'La notice ne doit prétendre aucune confiance publique ni inclure une clé privée.'
        }
        $cer = [Security.Cryptography.X509Certificates.X509Certificate2]::new((Join-Path $temporary $metadata.certificate.fileName))
        try {
            if ($cer.HasPrivateKey -or -not [Security.Cryptography.CryptographicOperations]::FixedTimeEquals($cer.RawData, $certificate.RawData)) {
                throw 'Seul le certificat DER public exact doit être distribué.'
            }
            $hash = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($cer.RawData))
            if ($metadata.certificate.sha256 -cne $hash) { throw 'Empreinte publique incorrecte.' }
        }
        finally { $cer.Dispose() }
    }
    Write-WindowsSelfSignedReleaseNotice -ReleaseDirectory $temporary -Rid 'win-x64' -Version '2.0.7' `
        -Certificate $certificate -EphemeralSigningIdentity $true
    $notice = Get-Content -LiteralPath (Join-Path $temporary 'WINDOWS-SIGNATURE-win-x64.json') -Raw | ConvertFrom-Json
    if (-not $notice.ephemeralSigningIdentity) { throw 'Une identité éphémère doit être explicitement annoncée.' }

    if ($IsWindows) {
        $prepare = Join-Path $scripts 'prepare-windows-self-signed-release.ps1'
        $pfxPath = Join-Path $temporary 'fixture.pfx'
        [IO.File]::WriteAllBytes($pfxPath, $certificate.Export([Security.Cryptography.X509Certificates.X509ContentType]::Pfx, $password))
        Assert-Rejected { & $prepare -PfxPath $pfxPath -PfxPassword $password -ExpectedThumbprint ('0' * 40) `
            -PublicCertificatePath (Join-Path $temporary 'wrong.cer') } 'ne correspond pas à ExpectedThumbprint'
        if (Test-Path "Cert:/CurrentUser/My/$($certificate.Thumbprint)") { throw 'Le mauvais pin ne doit rien importer.' }

        $secondRequest = [Security.Cryptography.X509Certificates.CertificateRequest]::new('CN=Additional PFX fixture', $rsa,
            [Security.Cryptography.HashAlgorithmName]::SHA256, [Security.Cryptography.RSASignaturePadding]::Pkcs1)
        $secondCertificate = $secondRequest.Create($secondRequest.SubjectName, $generator, (Get-Date).AddMinutes(-1),
            (Get-Date).AddDays(1), [Security.Cryptography.RandomNumberGenerator]::GetBytes(16))
        try {
            $collection = [Security.Cryptography.X509Certificates.X509Certificate2Collection]::new()
            $null = $collection.Add($certificate)
            $null = $collection.Add($secondCertificate)
            $multiplePfx = Join-Path $temporary 'multiple.pfx'
            [IO.File]::WriteAllBytes($multiplePfx, $collection.Export([Security.Cryptography.X509Certificates.X509ContentType]::Pfx, $fixturePassword))
            Assert-Rejected { & $prepare -PfxPath $multiplePfx -PfxPassword $password -ExpectedThumbprint $certificate.Thumbprint `
                -PublicCertificatePath (Join-Path $temporary 'multiple.cer') } 'exactement un certificat'
            if ((Test-Path "Cert:/CurrentUser/My/$($certificate.Thumbprint)") -or
                (Test-Path "Cert:/CurrentUser/My/$($secondCertificate.Thumbprint)")) { throw 'Un PFX multiple ne doit rien importer.' }
        }
        finally { $secondCertificate.Dispose() }

        # Échec après import : le certificat ET son nouveau conteneur CNG doivent disparaître.
        $capture = [pscustomobject]@{ KeyName = $null }
        Set-Item Function:/Export-Certificate -Value ({
            param($Cert, $FilePath, $Type)
            $key = [Security.Cryptography.X509Certificates.RSACertificateExtensions]::GetRSAPrivateKey($Cert)
            try { $capture.KeyName = $key.Key.KeyName }
            finally { $key.Dispose() }
            throw 'Fixture : export public interrompu.'
        }.GetNewClosure())
        try {
            Assert-Rejected { & $prepare -PfxPath $pfxPath -PfxPassword $password -ExpectedThumbprint $certificate.Thumbprint `
                -PublicCertificatePath (Join-Path $temporary 'failed-export.cer') } 'export public interrompu'
        }
        finally { Remove-Item Function:/Export-Certificate }
        if (-not $capture.KeyName -or (Test-Path "Cert:/CurrentUser/My/$($certificate.Thumbprint)") -or
            [Security.Cryptography.CngKey]::Exists($capture.KeyName, [Security.Cryptography.CngProvider]::MicrosoftSoftwareKeyStorageProvider)) {
            throw "Un échec d'export public doit supprimer le certificat et sa nouvelle clé CNG."
        }
        $created = & $prepare -PfxPath $pfxPath -PfxPassword $password -ExpectedThumbprint $certificate.Thumbprint `
            -PublicCertificatePath (Join-Path $temporary 'imported.cer')
        $imported = Get-DevelopmentCodeSigningCertificate -Thumbprint $created.Thumbprint -RequirePrivateKey
        $exportRejected = $false
        try { $null = $imported.Export([Security.Cryptography.X509Certificates.X509ContentType]::Pfx, $password) }
        catch {
            if ($_.Exception.GetBaseException() -is [Security.Cryptography.CryptographicException]) { $exportRejected = $true }
            else { throw }
        }
        if (-not $exportRejected) { throw 'La clé importée ne doit pas être exportable.' }
        Assert-Rejected { & $prepare -PfxPath $pfxPath -PfxPassword $password -ExpectedThumbprint $certificate.Thumbprint `
            -PublicCertificatePath (Join-Path $temporary 'duplicate.cer') } 'existe déjà'
        $null = Get-DevelopmentCodeSigningCertificate -Thumbprint $created.Thumbprint -RequirePrivateKey
        if ((Test-Path "Cert:/CurrentUser/Root/$($created.Thumbprint)") -or
            (Test-Path "Cert:/CurrentUser/TrustedPublisher/$($created.Thumbprint)")) { throw 'Aucune confiance ne doit être importée.' }
        $fixture = Join-Path $temporary 'fixture.exe'
        Copy-Item (Join-Path $env:SystemRoot 'System32/where.exe') $fixture
        $signTool = Get-WindowsSignTool
        & $signTool sign /s My /sha1 $created.Thumbprint /fd SHA256 /tr http://timestamp.acs.microsoft.com /td SHA256 $fixture
        Assert-DevelopmentAuthenticode -Path $fixture -Certificate $imported
        Write-Host 'PASS : PFX épinglé importé CNG non exportable, mauvaise identité/PFX multiple/remplacement refusés, cleanup certificat+clé après erreur, véritable signature horodatée ; aucune confiance importée.'
    }
    Write-Host 'PASS : notices autosignées x64/arm64 explicites, empreinte DER publique exacte ; aucune clé privée distribuée.'
}
finally {
    if ($imported) { Remove-Item -LiteralPath "Cert:/CurrentUser/My/$($imported.Thumbprint)" -DeleteKey -Force; $imported.Dispose() }
    if ($certificate) { $certificate.Dispose() }
    $rsa.Dispose()
    $password.Dispose()
    Remove-Item -LiteralPath $temporary -Recurse -Force
}
