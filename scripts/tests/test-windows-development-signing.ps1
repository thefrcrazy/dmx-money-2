# Test Windows réel : signature locale non approuvée, clé non exportable, digest PE et CMS.
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
if (-not $IsWindows) { throw 'Ce test Authenticode nécessite Windows.' }
$scripts = Split-Path -Parent $PSScriptRoot
. (Join-Path $scripts 'windows-development-signing.ps1')
$signTool = Get-WindowsSignTool
$temporary = Join-Path ([IO.Path]::GetTempPath()) ('dmx-signing-test-' + [guid]::NewGuid().ToString('N'))
$null = New-Item -ItemType Directory -Path $temporary
$certificate = $null
function Assert-Rejected([scriptblock]$Action, [string]$Expected) {
    try { & $Action }
    catch {
        if ($_.Exception.Message -like "*$Expected*") { return }
        throw "Erreur inattendue : $($_.Exception.Message)"
    }
    throw "La vérification aurait dû refuser : $Expected"
}
function Get-PeCmsOffsets([byte[]]$Bytes) {
    $pe = [BitConverter]::ToInt32($Bytes, 0x3c)
    $optional = $pe + 24
    $directories = if ([BitConverter]::ToUInt16($Bytes, $optional) -eq 0x20b) { $optional + 112 } else { $optional + 96 }
    $security = [BitConverter]::ToUInt32($Bytes, $directories + 4 * 8)
    $certificateSize = [BitConverter]::ToUInt32($Bytes, $security)
    $cms = $security + 8
    $lengthByte = $Bytes[$cms + 1]
    $headerSize = 2
    $contentSize = [uint32]$lengthByte
    if ($lengthByte -band 0x80) {
        $lengthBytes = $lengthByte -band 0x7f
        if ($lengthBytes -lt 1 -or $lengthBytes -gt 4) { throw 'Longueur DER invalide.' }
        $headerSize += $lengthBytes
        $contentSize = 0
        for ($i = 0; $i -lt $lengthBytes; $i++) { $contentSize = ($contentSize -shl 8) -bor $Bytes[$cms + 2 + $i] }
    }
    $lastCmsByte = $cms + $headerSize + $contentSize - 1
    if ($lastCmsByte -ge $security + $certificateSize -or $lastCmsByte -ge $Bytes.Length) { throw 'Fixture CMS invalide.' }
    return @{ Directories = $directories; Security = $security; LastCmsByte = $lastCmsByte }
}
try {
    $created = & (Join-Path $scripts 'new-windows-development-certificate.ps1') `
        -PublicCertificatePath (Join-Path $temporary 'fixture.cer') -ValidDays 1
    $certificate = Get-DevelopmentCodeSigningCertificate -Thumbprint $created.Thumbprint -RequirePrivateKey
    if ((Test-Path "Cert:/CurrentUser/Root/$($certificate.Thumbprint)") -or
        (Test-Path "Cert:/CurrentUser/TrustedPublisher/$($certificate.Thumbprint)")) {
        throw 'Le test ne doit importer aucune confiance.'
    }
    $chain = [Security.Cryptography.X509Certificates.X509Chain]::new()
    try {
        $chain.ChainPolicy.RevocationMode = [Security.Cryptography.X509Certificates.X509RevocationMode]::NoCheck
        if ($chain.Build($certificate) -or -not ($chain.ChainStatus | Where-Object {
            $_.Status -band [Security.Cryptography.X509Certificates.X509ChainStatusFlags]::UntrustedRoot
        })) { throw 'La fixture doit réellement être autosignée et non approuvée.' }
    }
    finally { $chain.Dispose() }
    $fixture = Join-Path $temporary 'fixture.exe'
    Copy-Item (Join-Path $env:SystemRoot 'System32/where.exe') $fixture
    # Aucune exécution de ce binaire ; les premiers contrôles d'intégrité sont hors réseau.
    & $signTool sign /s My /sha1 $certificate.Thumbprint /fd SHA256 $fixture
    Assert-DevelopmentAuthenticode -Path $fixture -Certificate $certificate -AllowMissingTimestamp
    Assert-Rejected { Assert-DevelopmentAuthenticode -Path $fixture -Certificate $certificate } 'Horodatage Authenticode absent'

    $rsa = [Security.Cryptography.RSA]::Create(3072)
    try {
        $request = [Security.Cryptography.X509Certificates.CertificateRequest]::new('CN=Different signer', $rsa,
            [Security.Cryptography.HashAlgorithmName]::SHA256, [Security.Cryptography.RSASignaturePadding]::Pkcs1)
        $otherCertificate = $request.CreateSelfSigned((Get-Date).AddMinutes(-1), (Get-Date).AddDays(1))
        try {
            Assert-Rejected { Assert-DevelopmentAuthenticode -Path $fixture -Certificate $otherCertificate -AllowMissingTimestamp } 'certificat épinglé'
            Assert-Rejected { [DmxMoney.DevelopmentAuthenticode]::VerifyFile($fixture, $otherCertificate) } 'Signature CMS épinglée'
        }
        finally { $otherCertificate.Dispose() }
    }
    finally { $rsa.Dispose() }

    $tampered = Join-Path $temporary 'tampered.exe'
    Copy-Item $fixture $tampered
    $bytes = [IO.File]::ReadAllBytes($tampered)
    $pe = [BitConverter]::ToInt32($bytes, 0x3c)
    $section = $pe + 24 + [BitConverter]::ToUInt16($bytes, $pe + 20)
    $raw = [BitConverter]::ToUInt32($bytes, $section + 20)
    if ($raw -ge $bytes.Length) { throw 'Fixture PE invalide.' }
    $bytes[$raw] = $bytes[$raw] -bxor 1
    [IO.File]::WriteAllBytes($tampered, $bytes)
    Assert-Rejected { [DmxMoney.DevelopmentAuthenticode]::VerifyFile($tampered, $certificate) } 'Digest PE Authenticode incorrect'

    # Altère le CMS dans la table WIN_CERTIFICATE, sans modifier le digest PE lui-même.
    $cmsTampered = Join-Path $temporary 'cms-tampered.exe'
    $bytes = [IO.File]::ReadAllBytes($fixture)
    $offsets = Get-PeCmsOffsets $bytes
    $directories = $offsets.Directories
    $security = $offsets.Security
    $unsigned = Join-Path $temporary 'unsigned.exe'
    $unsignedBytes = $bytes[0..($security - 1)]
    for ($i = 0; $i -lt 8; $i++) { $unsignedBytes[$directories + 4 * 8 + $i] = 0 }
    [IO.File]::WriteAllBytes($unsigned, $unsignedBytes)
    Assert-Rejected { [DmxMoney.DevelopmentAuthenticode]::VerifyFile($unsigned, $certificate) } 'Signature embarquée'
    # Dernier octet DER du PKCS#7, avant l'éventuel padding de WIN_CERTIFICATE.
    $lastCmsByte = $offsets.LastCmsByte
    $bytes[$lastCmsByte] = $bytes[$lastCmsByte] -bxor 1
    [IO.File]::WriteAllBytes($cmsTampered, $bytes)
    $rejected = $false
    try { [DmxMoney.DevelopmentAuthenticode]::VerifyFile($cmsTampered, $certificate) }
    catch { $rejected = $true }
    if (-not $rejected) { throw 'Le CMS modifié aurait dû être refusé.' }
    $timestamped = Join-Path $temporary 'timestamped.exe'
    Copy-Item $fixture $timestamped
    & $signTool sign /s My /sha1 $certificate.Thumbprint /fd SHA256 `
        /tr http://timestamp.acs.microsoft.com /td SHA256 $timestamped
    Assert-DevelopmentAuthenticode -Path $timestamped -Certificate $certificate
    # Régression de scope : un certificat hérité ne doit jamais ouvrir le mode dev en PublicTrust.
    $publicDirectory = Join-Path $temporary 'public-trust-rejection'
    $null = New-Item -ItemType Directory -Path $publicDirectory
    Copy-Item $timestamped (Join-Path $publicDirectory 'DmxMoney-win-x64-Setup.exe')
    $developmentCertificate = $certificate
    $publicRejected = $false
    try {
        & (Join-Path $scripts 'verify-windows-release.ps1') -ReleaseDirectory $publicDirectory `
            -Rid win-x64 -SignToolPath $signTool
    }
    catch {
        if ($_.Exception -is [System.Management.Automation.NativeCommandExitException] -and $_.Exception.ExitCode -eq 1) {
            $publicRejected = $true
        }
        else { throw }
    }
    finally { $developmentCertificate = $null }
    if (-not $publicRejected) { throw 'PublicTrust aurait dû refuser la fixture autosignée malgré le certificat hérité.' }
    $bytes = [IO.File]::ReadAllBytes($timestamped)
    $offsets = Get-PeCmsOffsets $bytes
    # Le jeton RFC 3161 est le dernier attribut non signé de cette fixture SignTool.
    $bytes[$offsets.LastCmsByte] = $bytes[$offsets.LastCmsByte] -bxor 1
    $timestampTampered = Join-Path $temporary 'timestamp-tampered.exe'
    [IO.File]::WriteAllBytes($timestampTampered, $bytes)
    # La signature CMS du fichier reste correcte : seule l'intégrité du jeton TSA change.
    [DmxMoney.DevelopmentAuthenticode]::VerifyFile($timestampTampered, $certificate, $false)
    Assert-Rejected { [DmxMoney.DevelopmentAuthenticode]::VerifyFile($timestampTampered, $certificate) } 'Horodatage RFC 3161 incorrect'
    Write-Host 'PASS : signature locale non approuvée, refus PublicTrust malgré le scope hérité, mauvais certificat, fichier non signé, PE/CMS modifiés, timestamp réel/absent/modifié ; aucune confiance importée.'
}
finally {
    if ($certificate) { Remove-Item -LiteralPath "Cert:/CurrentUser/My/$($certificate.Thumbprint)" -DeleteKey -Force }
    Remove-Item -LiteralPath $temporary -Recurse -Force
}
