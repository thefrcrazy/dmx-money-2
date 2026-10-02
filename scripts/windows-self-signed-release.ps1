# Métadonnées publiques uniquement ; aucun PFX, clé privée ou import de confiance.
function Write-WindowsSelfSignedReleaseNotice {
    param(
        [Parameter(Mandatory)][string]$ReleaseDirectory,
        [Parameter(Mandatory)][ValidateSet('win-x64', 'win-arm64')][string]$Rid,
        [Parameter(Mandatory)][string]$Version,
        [Parameter(Mandatory)][Security.Cryptography.X509Certificates.X509Certificate2]$Certificate,
        [bool]$EphemeralSigningIdentity
    )
    $certificateName = "DmxMoney-$Rid-self-signed.cer"
    $publicCertificate = $Certificate.Export([Security.Cryptography.X509Certificates.X509ContentType]::Cert)
    [IO.File]::WriteAllBytes((Join-Path $ReleaseDirectory $certificateName), $publicCertificate)
    $certificateHash = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($publicCertificate))
    $metadata = [ordered]@{
        schemaVersion = 1
        version = $Version
        architecture = $Rid
        signingMode = 'self-signed'
        publiclyTrustedPublisher = $false
        smartAppControlCompatibilityGuaranteed = $false
        ephemeralSigningIdentity = $EphemeralSigningIdentity
        privateKeyIncluded = $false
        certificate = [ordered]@{
            fileName = $certificateName
            subject = $Certificate.Subject
            issuer = $Certificate.Issuer
            sha1Thumbprint = $Certificate.Thumbprint
            sha256 = $certificateHash
            notBeforeUtc = $Certificate.NotBefore.ToUniversalTime().ToString('O')
            notAfterUtc = $Certificate.NotAfter.ToUniversalTime().ToString('O')
        }
    }
    $metadata | ConvertTo-Json -Depth 4 |
        Set-Content -LiteralPath (Join-Path $ReleaseDirectory "WINDOWS-SIGNATURE-$Rid.json") -Encoding utf8NoBOM
    $lifecycle = if ($EphemeralSigningIdentity) {
        'La clé est éphémère et sera supprimée après le build. Les prochaines publications peuvent avoir un autre certificat, y compris pour une autre architecture. Aucune continuité de confiance locale ne doit être supposée.'
    } else {
        'Le certificat est conservé par le responsable de publication. Utiliser la même clé pour les prochaines versions ; sa conservation et son renouvellement relèvent du responsable de publication.'
    }
    @"
DmxMoney $Version — Windows $Rid — SIGNATURE AUTOSIGNÉE

Ce paquet porte une vraie signature Authenticode SHA-256 et un horodatage RFC 3161,
vérifiés cryptographiquement. L'identité de l'éditeur n'est pas validée par une
autorité publique. Smart App Control peut bloquer l'installation ou l'exécution ;
cette publication ne garantit pas sa compatibilité. Les privilèges administrateur
ne fournissent pas d'exception SAC par application.

Certificat : $($Certificate.Subject)
Empreinte SHA-256 du certificat public : $certificateHash
$lifecycle

Le fichier $certificateName contient uniquement le certificat public, aucune clé
privée. Les scripts n'importent aucune confiance et ne désactivent aucune protection.
Le publier ne prouve pas à lui seul l'authenticité : obtenir son empreinte depuis une
source de publication déjà connue. Les noms Setup et flux Velopack sont conservés
pour les mises à jour ; celles-ci vérifient taille et SHA-256 du paquet complet.
"@ | Set-Content -LiteralPath (Join-Path $ReleaseDirectory "WINDOWS-SIGNATURE-$Rid.txt") -Encoding utf8NoBOM
}
