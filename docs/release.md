# Publier une version de DmxMoney 2

Deux workflows GitHub Actions :

* **`ci.yml`** — à chaque push et pull request : noyau Rust (format, clippy, tests), app Linux
  (clippy, build release, validation `.desktop`/AppStream), app Apple (bindings, tests DmxKit,
  builds macOS modern et legacy, iOS simulateur), app Windows (bindings, build, tests xUnit,
  ouverture de chaque page de l'app publiée). Les étapes de vérification échouent si les
  bindings Swift/C# ou les icônes natives (`scripts/gen-native-icons.py --check`) n'ont pas été
  régénérés après une modification du noyau ou de `shared/icons/native.json`.
* **`release.yml`** — sur un tag `v2.*` (ou déclenchement manuel) : deux DMG macOS signés et
  notarisés (`-apple-silicon` et `-intel-catalina`) + flux `updates.json`, installeurs Velopack
  x64 et arm64, AppImage, bundle Flatpak, puis création de la release GitHub avec les notes
  tirées du `CHANGELOG.md`.

## Vérification des interfaces à chaque version

Toute modification d’une interface doit être comparée aux implémentations macOS moderne,
macOS Intel/Catalina, Windows, Linux et PWA. Documenter les différences voulues et reporter
les corrections communes, sans supposer que la compilation d’une variante valide les autres.

- Vérifier les actions accessibles (onglets, mise à jour, import/export), les soldes et les formats.
- Contrôler les icônes en clair, sombre et après retour au clair, y compris une sélection active.
- La CI lance `scripts/verify-apple-ui.sh` : application AppKit, données fictives isolées,
  icônes embarquées forcées, navigation effective dans les quatre onglets des paramètres.
  Les captures sont conservées dans l’artefact `apple-ui-catalina-artwork` pour revue visuelle.
- Ce test sur un OS récent ne remplace pas un essai sur Catalina réel. Signaler explicitement
  lorsque l’ancien OS n’est pas disponible ; ne pas qualifier cette limite de validation Catalina.
- Conserver la cible Intel macOS 10.15. Le SDK local Xcode 27 peut nécessiter une cible de test
  12.0 (`DMXMONEY_UI_MIN_MACOS=12.0`), sans changer la cible des binaires distribués.

## Étapes

```bash
# 1. Mettre à jour la version partout (fichier VERSION = source de vérité)
echo 2.0.1 > VERSION
#    apple/project.yml (MARKETING_VERSION), linux/dmx-money-gtk/Cargo.toml, windows/…/Version
# 2. Compléter le CHANGELOG (section « ## 2.0.1 »)
# 3. Vérifier localement
cargo fmt --all --check && cargo clippy --workspace --all-targets && cargo test --workspace
# 4. Taguer
git tag v2.0.1 && git push origin v2.0.1
```

## Secrets attendus

Aucun secret n'est nécessaire pour la CI. La release Windows exige une identité de signature
configurée dans l'environnement GitHub `windows-signing` : Public Trust par défaut, ou
autosignée stable avec le mode `self-signed` choisi explicitement. Sans l'identité du mode
choisi, la publication échoue. Les secrets Apple restent optionnels ; leur absence produit
des builds macOS sans identité Developer ID.

| Secret | Usage |
|---|---|
| `DMXMONEY_MANAGED_BRIDGE_REGISTRATION_SECRET` | secret d'enregistrement du pont managé, compilé dans le noyau (`option_env!`) |
| `DMXMONEY_MANAGED_BRIDGE_URL` | URL du Worker Cloudflare du pont managé |
| `APPLE_CERTIFICATE_P12` / `APPLE_CERTIFICATE_PASSWORD` | certificat Developer ID (base64) et son mot de passe |
| `APPLE_TEAM_ID` | équipe de signature ; active aussi les entitlements iCloud |
| `DMX_ICLOUD_CONTAINER` | conteneur CloudKit (`iCloud.com.dmxmoney.app`) |
| `APPLE_API_KEY` / `APPLE_API_KEY_ID` / `APPLE_API_ISSUER` | clé App Store Connect (base64) pour la notarisation |
| `DMX_UPDATE_FEED_URL` | URL du flux lu par l'app macOS, par exemple `https://github.com/<owner>/<repo>/releases/latest/download/updates.json` |

Sans ce secret, les builds utilisent le flux des releases GitHub du dépôt. Les deux variantes
macOS partagent `FileActions.settingsActions` : le bouton « Vérifier les mises à jour… »
reste accessible dans les paramètres et dans le menu de l’application.

## Mises à jour macOS

`scripts/update-feed.py` produit `updates.json`, publié comme asset de la release :

```json
{
  "version": "2.0.1",
  "notes": "- …",
  "platforms": {
    "darwin-arm64":  { "url": "https://…/DmxMoney-2.0.1-apple-silicon.dmg",  "minimumSystemVersion": "26.0" },
    "darwin-x86_64": { "url": "https://…/DmxMoney-2.0.1-intel-catalina.dmg", "minimumSystemVersion": "10.15" }
  }
}
```

L'app compare cette version à la sienne (au plus une vérification par jour, plus l'entrée de
menu), affiche les nouveautés puis permet de télécharger et remplacer l’application avec
copie de secours. L’installation manuelle du DMG reste disponible. Sparkle 2 n'est pas utilisé : ses binaires officiels exigent macOS 11, ce qui
empêcherait le lancement sous Catalina.

## Windows

`scripts/build-windows.ps1` produit la publication autonome puis l'installeur Velopack, avec
un canal distinct `win-x64` / `win-arm64`. L'updater consulte le dépôt V2
`https://github.com/thefrcrazy/dmx-money-2`. Les préversions sont désactivées par défaut sur
une version stable ; les installations de préversion les activent par défaut. Le premier clic
vérifie la disponibilité ; « Installer » confirme le téléchargement et le redémarrage.
`DMXMONEY_UPDATE_URL` accepte une source personnalisée HTTPS sans identifiants ni paramètres.
L'updater télécharge le paquet complet puis vérifie lui-même la taille et l'empreinte SHA-256
du flux, même si Velopack réutilise un fichier déjà en cache. Le flux de lecture reste ouvert
avec `FileShare.Read` jusqu'au lancement d'Update.exe, qui attend la sortie du parent sous Windows.
Un cache invalide est retiré pour permettre un nouveau téléchargement à la prochaine tentative.
Les deltas sont désactivés : leur reconstruction recomprime le ZIP et ne garantit pas une
empreinte identique à celle du paquet complet. Les téléchargements peuvent donc être plus gros.
Cette empreinte protège l'intégrité ; elle ne remplace pas la signature du code.
Sources de la version épinglée :
[réutilisation du cache et contrôle des paquets](https://github.com/velopack/velopack/blob/ed8600eee530a38d2669444b03eb240e56bc5aa3/src/lib-csharp/UpdateManager.cs),
[recompression des deltas](https://github.com/velopack/velopack/blob/ed8600eee530a38d2669444b03eb240e56bc5aa3/src/bins/src/commands/patch.rs).

### Signature Windows et Smart App Control

Le message « éditeur invérifiable » de Smart App Control concerne la confiance dans les
binaires distribués. Il faut une vraie identité Authenticode publique ; un nom d'éditeur dans
le projet ou un certificat auto-signé ne corrige pas ce blocage. La signature doit couvrir
l'application, le noyau Rust, les DLL et les exécutables générés par Velopack, dont Setup et
Update. Voir [Microsoft : Smart App Control](https://learn.microsoft.com/en-us/windows/apps/develop/smart-app-control/overview).

Le workflow choisit explicitement la variable de dépôt `WINDOWS_SIGNING_MODE` :
`public-trust` par défaut, ou `self-signed` pour une publication gratuite
avec une identité autosignée stable. Les deux modes refusent une configuration absente et
ne basculent jamais automatiquement vers un paquet non signé. Le mode autosigné est décrit
ci-dessous ; il ne nécessite aucun abonnement Azure.

Le mode `public-trust` utilise **Azure Artifact Signing**, profil **Public Trust**, avec
authentification OIDC et aucun export de clé privée. Cette intégration est optionnelle : la
validation d'identité Microsoft et le compte Azure doivent être configurés réellement avant
de l'utiliser. Un autre certificat d'autorité publique nécessite sa propre intégration :

Public Trust accepte les organisations de l’Union européenne ; les validations individuelles
sont actuellement limitées aux États-Unis et au Canada. Pour Developmax, utiliser l’identité
légale justifiée de l’organisation, sous réserve d’acceptation par Microsoft. Le nom d’un
certificat autosigné ne remplace pas cette validation. Microsoft affiche Basic à
9,99 USD/mois pour 5 000 signatures, avec tarification locale à vérifier avant souscription.
Aucun abonnement de signature n’a été créé pendant cet audit. Sources :
[éligibilité](https://learn.microsoft.com/en-us/azure/artifact-signing/quickstart),
[tarif](https://learn.microsoft.com/en-us/azure/artifact-signing/how-to-change-sku).

Une autre voie sans abonnement payant est le programme Open Source de
[SignPath Foundation](https://signpath.org/terms.html), sous acceptation du projet.
Il exige notamment une licence OSI sans double licence commerciale, un projet maintenu
et déjà publié, une origine de build vérifiable et une approbation manuelle des signatures.
Le certificat porte l’identité de **SignPath Foundation**, pas celle de Developmax.
Ce programme n’est pas configuré ici et aucune candidature n’a été envoyée ; il ne faut
pas l’assimiler à une signature gratuite automatiquement disponible pour ce dépôt.

1. Créer le compte Artifact Signing, faire valider l'identité et créer un profil Public Trust.
2. Créer l'environnement GitHub `windows-signing`. Configurer une identité Azure fédérée dont
   le sujet est `repo:thefrcrazy/dmx-money-2:environment:windows-signing`, audience
   `api://AzureADTokenExchange`, et limiter l'environnement aux références de release voulues.
3. Donner à cette identité le rôle **Artifact Signing Certificate Profile Signer** uniquement
   sur le profil concerné (les installations Azure plus anciennes affichent « Trusted Signing
   Certificate Profile Signer »). Aucun rôle de gestion de l'abonnement n'est nécessaire.
4. Ajouter les variables de cet environnement :

| Variable | Valeur |
|---|---|
| `AZURE_CLIENT_ID` | identifiant de l'application Azure fédérée |
| `AZURE_TENANT_ID` | identifiant du tenant |
| `AZURE_SUBSCRIPTION_ID` | identifiant de l'abonnement contenant le compte |
| `WINDOWS_SIGNING_ENDPOINT` | endpoint HTTPS de la région, par exemple `https://weu.codesigning.azure.net` |
| `WINDOWS_SIGNING_ACCOUNT` | nom du compte Artifact Signing |
| `WINDOWS_SIGNING_PROFILE` | nom du profil Public Trust validé |

`prepare-windows-signing.ps1` utilise le SignTool x64 du SDK Windows et le client Microsoft
Artifact Signing `1.0.128`, dont il vérifie la signature NuGet. Les signatures Authenticode et
leur horodatage SHA-256 sont ajoutés par Velopack pendant l'empaquetage. Sa version `0.0.1298`
ne fournit pas le dlib Azure : le script lui passe un template SignTool avec les chemins
absolus du SDK et du client Microsoft. `verify-windows-release.ps1` vérifie ensuite la chaîne
de confiance et l'horodatage de l'installeur et de chaque EXE/DLL du paquet complet, ainsi que
les tailles et empreintes du flux ; tout échec bloque la publication. Les builds locaux sans
`-RequireSigning` restent possibles pour le développement.

Tester ensuite une vraie installation et une mise à jour sous Windows 11 avec Smart App
Control actif, pour x64 et arm64. Le runner et macOS ne remplacent pas cette vérification.
La signature n'offre pas une garantie d'absence d'avertissements SmartScreen : sa réputation
se construit séparément, même avec EV ou Artifact Signing. Ne pas désactiver les protections
Windows pour résoudre un défaut de distribution. Sources :
[intégration Artifact Signing](https://learn.microsoft.com/en-us/azure/artifact-signing/how-to-signing-integrations),
[réputation SmartScreen](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation),
[tests Smart App Control](https://learn.microsoft.com/en-us/windows/apps/develop/smart-app-control/test-your-app-with-smart-app-control).

### Certificat autosigné pour le développement Windows

Ce mode crée une vraie signature locale pour tester DmxMoney et ses paquets de mise à jour.
L’identité `Developmax / Collignon Maxim` est une déclaration locale, sans validation d’une
autorité publique. Un compte administrateur ne fournit pas d’exception Smart App Control
par application : ce certificat ne garantit donc pas que Windows autorisera l’installation
ou l’exécution. Aucun script ne désactive SAC/Defender, n’élève l’application ni n’importe
automatiquement de certificat dans `Root` ou `TrustedPublisher`. Source :
[FAQ Smart App Control](https://support.microsoft.com/en-us/windows/what-is-smart-app-control-285ea03d-fa88-4d56-882e-6698afdb7003).

Depuis **PowerShell 7 sous Windows**, avec SDK Windows, Rust, .NET et Velopack installés :

```powershell
# Une fois : clé RSA 3072 bits non exportable, magasin personnel de l'utilisateur.
# Le fichier .cer exporté ne contient que le certificat public.
$localCertificate = ./scripts/new-windows-development-certificate.ps1

# Chaque build de développement : app, DLL, moteur de mise à jour et installeur signés.
./scripts/build-windows.ps1 -Rid win-x64 -Version 2.0.7 `
    -DevelopmentCertificateThumbprint $localCertificate.Thumbprint

# Dans une autre session : relever l'empreinte du certificat personnel existant.
Get-ChildItem Cert:/CurrentUser/My -CodeSigningCert |
    Select-Object Subject, Thumbprint, NotAfter
```

Les résultats restent dans `target/windows/development-releases/win-x64` (ou `win-arm64`),
distincts des releases publiques. L’empaquetage conserve les flux Velopack et leurs
empreintes SHA-256 pour les essais de mise à jour ; utiliser le même certificat pour les
builds successifs. Une source de mise à jour personnalisée doit être servie en HTTPS et
configurée avec `DMXMONEY_UPDATE_URL`. Essayer effectivement installation et mise à jour
sur le PC ciblé avant d’affirmer leur compatibilité. Conserver la clé dans ce compte Windows :
elle n’est pas exportable en PFX. Le certificat expire après un an par défaut ; créer une
nouvelle identité locale ne renouvelle pas automatiquement la confiance des installations.

SignTool utilise `/sha1` uniquement pour sélectionner l’empreinte du certificat dans
`CurrentUser/My` ; le digest des fichiers et l’horodatage RFC 3161 utilisent **SHA-256**.
Velopack conserve les signatures fournisseurs déjà reconnues (Microsoft et son moteur de
mise à jour). Setup, les fichiers `DmxMoney*.exe` / `DmxMoney*.dll` et `dmx_ffi.dll` exigent
le certificat de développement épinglé. Les autres binaires, dont Squirrel/Update, exigent
soit ce même certificat, soit une signature publique vérifiée par SignTool `/pa /all /tw`.
Aucun autre certificat autosigné non approuvé n'est accepté.

Pour la signature de développement, le contrôle local compare le certificat DER exact du signataire CMS embarqué, vérifie sa
signature avec la clé publique épinglée puis recalcule le digest Authenticode du PE via le
SIP Windows. Il vérifie aussi cryptographiquement le jeton d’horodatage RFC 3161 et son lien
avec la signature du fichier, ainsi que les tailles/SHA-256 des paquets. Ce contrôle local
ne valide pas la confiance publique de l’éditeur ni de l’autorité d’horodatage.
`-RequireSigning` et les paramètres Artifact Signing refusent toute combinaison avec
`-DevelopmentCertificateThumbprint` et `-SelfSignedRelease` ; le mode `public-trust` continue
d’exiger une chaîne approuvée avec SignTool `/pa`.

### Publication gratuite autosignée avec identité stable

Le mode `self-signed` réutilise le même certificat RSA 3072 bits et sa clé pour x64, arm64
et les versions suivantes. Une nouvelle clé à chaque release ferait tourner l'identité
et ne préserverait aucune confiance locale accordée au certificat précédent. Conserver
un PFX **chiffré**, hors du dépôt et des assets publics, ainsi que son mot de passe séparé.
Le certificat exporté par `new-windows-development-certificate.ps1` a une clé non exportable :
ce script local ne peut pas fournir le PFX persistant destiné à la CI.

Après autorisation du responsable de publication, définir `WINDOWS_SIGNING_MODE=self-signed`
comme **variable du dépôt** : le mode est évalué dès la préparation du job. Les variables
d'environnement ne sont disponibles qu'après son démarrage
([GitHub : disponibilité des variables](https://docs.github.com/en/actions/reference/workflows-and-actions/variables#configuration-variable-precedence)).
Configurer les autres valeurs dans l'environnement GitHub `windows-signing`, limité aux
références de publication autorisées :

| Configuration | Type | Contenu |
|---|---|---|
| `WINDOWS_SIGNING_MODE` | variable du dépôt | `self-signed` |
| `WINDOWS_SELF_SIGNED_THUMBPRINT` | variable publique | empreinte SHA-1 attendue du certificat, 40 caractères HEX |
| `WINDOWS_SELF_SIGNED_PFX` | secret | PFX chiffré encodé en Base64 |
| `WINDOWS_SELF_SIGNED_PASSWORD` | secret | mot de passe du PFX |

Le runner décode temporairement le PFX, appelle
`prepare-windows-self-signed-release.ps1 -PfxPath ... -PfxPassword <SecureString>
-ExpectedThumbprint ... -PublicCertificatePath ...`, puis supprime le fichier PFX.
Le préflight importe en mémoire sans persistance et exige un seul certificat avec sa clé,
le pin attendu, la validité courante, RSA 3072 minimum et l'usage Code Signing. L'import
persistant force CNG avec `PFXImportCertStore`, magasin `CurrentUser/My`, sans drapeau
d'exportabilité ni remplacement d'une clé existante. Le contrôle supplémentaire exige
`RSACng.ExportPolicy=None`. Un certificat déjà présent est refusé, sans être supprimé ; un
échec après création retire le certificat et sa nouvelle clé. Aucun magasin de confiance
n'est modifié. Source : [PFXImportCertStore](https://learn.microsoft.com/en-us/windows/win32/api/wincrypt/nf-wincrypt-pfximportcertstore).

La construction utilise `-DevelopmentCertificateThumbprint <pin> -SelfSignedRelease`.
Les résultats sont dans `target/windows/self-signed-releases/<rid>` et comprennent des
noms distincts par architecture : Setup, paquet complet, flux Velopack,
`DmxMoney-<rid>-self-signed.cer`, `WINDOWS-SIGNATURE-<rid>.json` et `.txt`. Ces notices
déclarent explicitement l'absence de confiance publique et de garantie SAC ; le CER contient
uniquement le certificat public. `-EphemeralSigningIdentity`, réservé à une publication
explicitement temporaire, doit accompagner `-SelfSignedRelease` et annonce la rotation de
l'identité. La CI de pull request teste uniquement ses propres fixtures temporaires.

Les scripts vérifient réellement le CMS épinglé, le digest PE et l'horodatage, puis les
tailles et SHA-256 des paquets. `--runtime <rid>` et `--channel <rid>` sont tous deux passés
à Velopack ; la vérification contrôle le RID et l'architecture du manifeste ainsi que les
en-têtes PE de l'app et du noyau natif. Les helpers Setup/Stub/Update Windows de Velopack
`0.0.1298` sont communs aux architectures : il ne faut pas les annoncer comme binaires
natifs ARM64. Le workflow supprime le certificat et sa clé privée du runner après le build.
L'identité stable est conservée dans les secrets de publication, jamais dans les artefacts.

Le test Windows ajoute un vrai import PFX CNG non exportable et une signature horodatée,
le refus d'un mauvais pin, d'un PFX contenant plusieurs certificats et d'un remplacement,
ainsi que le retrait certificat/clé après échec d'export public puis le réimport réussi.
Les tests portables vérifient la séparation des modes et les notices CER/JSON x64/arm64.

`pwsh scripts/tests/test-windows-release.ps1` contrôle les options sur toutes les plateformes.
Sous Windows, il crée en plus un certificat éphémère non approuvé, signe une copie de fixture
PE et vérifie le refus d’un mauvais certificat, d’un fichier non signé, d’un digest PE
modifié, d’un CMS modifié et d’un horodatage absent. Il signe aussi une fixture avec un
horodatage Microsoft RFC 3161 réel et vérifie le refus d’un jeton altéré et le refus de cette
fixture par le mode Public Trust, même avec une variable de développement héritée ; cette dernière
partie nécessite l’accès au service d’horodatage. Le certificat et sa clé sont supprimés
après ce test, sans modifier la confiance du système. Les builds distribuables de
développement exigent cet horodatage.

Références Microsoft :
[New-SelfSignedCertificate](https://learn.microsoft.com/en-us/powershell/module/pki/new-selfsignedcertificate?view=windowsserver2025-ps),
[vérification CMS](https://learn.microsoft.com/en-us/windows/win32/api/wincrypt/nf-wincrypt-cryptmsgcontrol),
[digest PE via CryptSIPVerifyIndirectData](https://learn.microsoft.com/en-us/windows/win32/api/mssip/nf-mssip-cryptsipverifyindirectdata),
[jeton RFC 3161](https://learn.microsoft.com/en-us/windows/win32/api/wincrypt/nf-wincrypt-cryptverifytimestampsignature).

## Linux

* **AppImage** : `scripts/build-linux-appimage.sh` (icônes, build release, AppDir, appimagetool).
* **Flatpak** : `linux/flatpak/com.dmxmoney.app.yml`. Le manifeste télécharge les dépendances
  Cargo pendant le build (`--share=network`) ; pour Flathub, générer `cargo-sources.json` avec
  `flatpak-builder-tools/cargo` et retirer ce réglage.

Le client PWA (`pwa/dist`) est construit avant l'installation pour être servi par le pont local ;
sans lui, le pont expose seulement l'API.
