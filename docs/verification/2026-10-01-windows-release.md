# Vérification Windows, distribution et compagnon — 1 octobre 2026

## Changements

- Les releases Windows exigent désormais une identité Azure Artifact Signing **Public Trust**
  réelle, avec authentification OIDC. La signature est effectuée par Velopack pendant
  l'empaquetage ; l'installeur et les EXE/DLL du paquet complet sont ensuite vérifiés avec
  SignTool, ainsi que l'horodatage et les tailles/empreintes SHA-256 du flux.
- Velopack `0.0.1298` expose l'option Azure, mais ne fournit pas son dlib : le script emploie
  son template de signature, le SignTool du SDK Windows et le client Microsoft Artifact
  Signing `1.0.128`. Aucune signature simulée n'est utilisée dans la distribution officielle.
- Un mode explicitement autosigné a été ajouté pour le développement Windows à la demande
  de l'utilisateur : identité locale `Developmax / Collignon Maxim`, clé RSA 3072 bits non
  exportable dans `CurrentUser/My`, export public `.cer` seulement, aucun ajout de confiance.
  Il est incompatible avec `-RequireSigning` et Artifact Signing, et produit ses paquets
  dans `target/windows/development-releases`. Setup et les fichiers DmxMoney/Rust exigent
  la signature CMS avec le certificat embarqué épinglé, le digest PE via le SIP Windows
  et le jeton d'horodatage RFC 3161. Les autres binaires, y compris Squirrel/Update,
  exigent soit cette signature de développement, soit une vraie signature publique
  vérifiée par SignTool `/pa /all /tw`, car Velopack conserve les signatures fournisseurs
  déjà approuvées. Aucun autre certificat autosigné non approuvé n'est accepté.
  Cette vérification d'intégrité ne certifie ni confiance publique ni autorisation SAC.
- L'updater cible V2, exige HTTPS et SHA-256, conserve les canaux x64/arm64 et distingue
  vérification, téléchargement et confirmation du redémarrage. Les installations stables
  n'activent plus les préversions par défaut.
- Les mises à jour téléchargent uniquement le paquet complet. L'application vérifie la
  taille et le SHA-256 avant application, y compris en cas de fichier déjà en cache, puis
  maintient le flux ouvert avec `FileShare.Read` jusqu'au lancement d'Update.exe. Un cache
  invalide est retiré pour le prochain téléchargement. Les deltas sont désactivés car leur
  reconstruction recomprime le ZIP et ne garantit pas l'empreinte du paquet complet.
  Sources épinglées :
  [cache et contrôle Velopack](https://github.com/velopack/velopack/blob/ed8600eee530a38d2669444b03eb240e56bc5aa3/src/lib-csharp/UpdateManager.cs),
  [reconstruction des deltas](https://github.com/velopack/velopack/blob/ed8600eee530a38d2669444b03eb240e56bc5aa3/src/bins/src/commands/patch.rs).
- Les empreintes du flux Velopack sont vérifiées sous leur format réel : **64 chiffres
  hexadécimaux SHA-256**. Le commentaire XML de `CalculateStreamSHA256` de Velopack
  `0.0.1298` indique à tort base64 ; son implémentation produit
  de l'hexadécimal. Source :
  [implémentation officielle](https://github.com/velopack/velopack/blob/0.0.1298/src/lib-csharp/Util/IoUtil.cs).
- Les interfaces du compagnon WinUI, GTK, SwiftUI moderne et DmxKit reconnaissent le relais
  distant par `/relay/`. Elles montrent connexion Internet et chiffrement entre appareils,
  sans attendre un certificat local ni exposer l'adresse du relais. La génération du QR
  reste conditionnée à la connexion effective du bureau.

## Vérifications achevées

- **54 tests .NET sur 54 réussis sur macOS et Windows**, aucun ignoré sous Windows : suite complète
  `windows/tests/DmxMoney.Tests`, incluant noyau/interops, modèles de vue, 37 contrôles de
  l'updater et 2 régressions du compagnon distant. Interops et modèles recompilés sur macOS.
  Six contrôles de flux couvrent taille, empreinte, cache altéré, verrou et annulation ;
  les noms de paquets invalides sont testés séparément. Le refus de suppression d'un fichier verrouillé a été réellement
  exécuté sous Windows ; le résultat macOS seul ne valide pas cette sémantique spécifique.
- **CI Windows x64 intégralement réussie** sur le commit `a2b2e1e` : compilation Rust,
  bindings C#, application WinUI, vrai empaquetage Velopack `0.0.1298`, suite .NET et
  navigation automatique dans toutes les pages. Le vérificateur a contrôlé **Setup et
  277 binaires** : **46 signatures de développement épinglées (Setup inclus)** et
  **232 signatures fournisseurs publiquement approuvées**, ainsi que les tailles et
  SHA-256 HEX du vrai flux de paquets. Le certificat et sa clé de test ont été supprimés
  en fin de build ; aucun certificat de confiance n'a été importé et aucune release
  officielle n'a été publiée. L'artefact uploadé contient la publication native d'origine,
  qui peut rester non signée : Velopack signe les copies de staging. Preuve :
  [job Windows du run 36890917651](https://github.com/thefrcrazy/dmx-money-2/actions/runs/36890917651/job/110466530551).
- Parsing PowerShell des six scripts de distribution/tests et compilation du helper C#
  Authenticode réussis sur macOS. Les 10 tests de refus d'options ont réussi : version
  contenant une injection, signature absente/incomplète, combinaison excluant la vérification
  de l'installeur, empreinte injectée, mélange autosigné/public et endpoint de signature HTTP.
  Ces contrôles portables n'exécutent pas les API cryptographiques Windows.
  Le runtime PowerShell portable officiel
  `7.6.6`, conservé dans `/tmp`, a été vérifié contre le SHA-256 de son asset GitHub.
- **Fixtures Authenticode réussies sous Windows** sur `a2b2e1e` : certificat RSA 3072
  non exportable réellement non approuvé, signature CMS épinglée et digest PE vérifiés,
  mauvais certificat et fichier non signé refusés, PE/CMS altérés refusés, horodatage
  RFC 3161 Microsoft réel accepté et jeton altéré refusé. Le mode Public Trust a refusé
  cette fixture autosignée malgré une variable de développement héritée. Aucun ajout de
  confiance. Preuve : étape « Contrôles des scripts de distribution Windows » du
  [job Windows 110466530551](https://github.com/thefrcrazy/dmx-money-2/actions/runs/36890917651/job/110466530551).
  Les deux premiers essais ont révélé le mode de décodage PKCS#7 et le diagnostic
  `CRYPT_E_ATTRIBUTES_MISSING`, corrigés sans réduire les contrôles d'intégrité.
- Le code officiel Velopack `0.0.1298` confirme que `SHA256` du flux est un **HEX de
  64 caractères**, malgré un commentaire XML obsolète indiquant base64. Le vérificateur
  de distribution et l'updater ont été corrigés pour ce format réel ; le vérificateur
  exige 64 caractères hexadécimaux ASCII et compare les empreintes sans sensibilité
  à la casse. Source :
  [IoUtil.CalculateStreamSHA256](https://github.com/velopack/velopack/blob/0.0.1298/src/lib-csharp/Util/IoUtil.cs).
- YAML CI/release chargé avec succès ; environnement `windows-signing`, permissions OIDC
  limitées au job Windows, runtimes .NET 8/10 et contrôle des scripts dans la CI vérifiés.
- Parsing Swift réussi pour `ModernCompanion.swift` et `DmxKit/Pages/SettingsPage.swift` ;
  fichier GTK `settings.rs` formaté avec rustfmt. Ces contrôles ne valident pas l'exécution UI.

Commande de la suite .NET :

```bash
dotnet test windows/tests/DmxMoney.Tests/DmxMoney.Tests.csproj \
  --no-restore -v minimal -m:1 -nr:false -p:UseSharedCompilation=false
```

## Contrôle NuGet

La restauration réelle du projet WinUI a réussi avec :

```bash
dotnet restore windows/src/DmxMoney.App/DmxMoney.App.csproj \
  -p:EnableWindowsTargeting=true -p:NuGetAudit=true -p:NuGetAuditMode=all \
  -m:1 -nr:false -v minimal
```

Aucun avertissement NU190x n'a été produit. Un second contrôle indépendant a comparé les
**25 couples package/version résolus**, directs et transitifs, extraits des `project.assets.json`
de tous les projets Windows, au flux officiel `VulnerabilityInfo` de nuget.org. Les plages
ont été évaluées par **NuGet.Versioning du SDK 10.0.300**, sans comparaison de versions
artisanale. Données obtenues : base mise à jour le **26 septembre 2026 à 05:43 UTC**, mise à
jour complémentaire du **1 octobre 2026 à 05:43 UTC**. **Aucune des 25 versions n'est dans
une plage déclarée vulnérable.** Cela couvre notamment WinAppSDK `1.7.250606001`, Velopack
`0.0.1298`, H.NotifyIcon `2.3.0`, QRCoder `1.6.0` et leurs dépendances résolues.

Le catalogue contient des avis pour Newtonsoft.Json et System.Drawing.Common, mais les
versions utilisées (`13.0.3` et `9.0.1`) sont hors des plages concernées. Cette vérification
constate les vulnérabilités connues du catalogue ; elle ne garantit pas l'absence de défaut
ni le support courant de chaque version. Le client de signature, le SDK .NET et les packs
de runtime distribués ne font pas partie de cet inventaire de 25 bibliothèques.
Source : [API officielle NuGet VulnerabilityInfo](https://learn.microsoft.com/en-us/nuget/api/vulnerability-info).

La commande `dotnet list … package --vulnerable --include-transitive --no-restore` échoue
toujours avec `Sequence contains no matching element`, même après la restauration auditée.
La nouvelle syntaxe .NET 10 et le framework explicite ne corrigent pas ce défaut de listing.
Ce résultat **n'est pas un échec de compilation WinUI** : aucune compilation n'est lancée par
ce contrôle, et un projet WinUI minimal isolé sur le même Mac se restaure et liste correctement
ses dépendances. Le problème se reproduit avec une copie du projet applicatif, même sans ses
références de projets. Le déclencheur exact de l'exception du reporting NuGet reste non établi.
L'audit par restauration et le contrôle du catalogue ci-dessus ont donc été utilisés.

## Vérifications restant nécessaires

- Pour une distribution avec signature publique, configurer le fournisseur reconnu choisi.
  La voie Azure nécessite la validation d'identité, le profil Public Trust, le rôle de
  signature et les variables GitHub décrits dans `docs/release.md`.
- Exécuter le pipeline de release avec cette vraie identité publique et essayer installation,
  désinstallation et mise à jour x64/arm64 sous Windows 11 avec Smart App Control actif.
  Le paquet autosigné de test et le runner Windows ne valident pas cette autorisation SAC.
- Exécuter GTK sur un bureau Linux réel et valider Windows arm64. La compilation GTK
  a réussi dans la CI ; elle ne valide pas son exécution graphique. La CI Windows x64
  réussie confirme compilation et démarrage des pages, mais ne remplace pas une revue visuelle
  ni les essais de synchronisation mobile ou de signature/distribution.

Smart App Control exige une signature publiquement reconnue lorsque la réputation ne suffit
pas. SmartScreen gère séparément sa réputation : une signature valide, y compris EV ou
Artifact Signing, ne garantit plus l'absence immédiate de ses avertissements. Aucun changement
ne désactive les protections Windows. Sources :
[Smart App Control](https://learn.microsoft.com/en-us/windows/apps/develop/smart-app-control/overview),
[réputation SmartScreen](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation),
[intégration Artifact Signing](https://learn.microsoft.com/en-us/azure/artifact-signing/how-to-signing-integrations).
