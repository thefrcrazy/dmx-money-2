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
  dans `target/windows/development-releases`. Il vérifie la signature CMS avec le certificat
  embarqué épinglé, le digest PE via le SIP Windows et le jeton d'horodatage RFC 3161.
  Cette vérification d'intégrité ne certifie ni confiance publique ni autorisation SAC.
- L'updater cible V2, exige HTTPS et SHA-256, conserve les canaux x64/arm64 et distingue
  vérification, téléchargement et confirmation du redémarrage. Les installations stables
  n'activent plus les préversions par défaut.
- Les interfaces du compagnon WinUI, GTK, SwiftUI moderne et DmxKit reconnaissent le relais
  distant par `/relay/`. Elles montrent connexion Internet et chiffrement entre appareils,
  sans attendre un certificat local ni exposer l'adresse du relais. La génération du QR
  reste conditionnée à la connexion effective du bureau.

## Vérifications achevées

- **41 tests .NET sur 41 réussis**, aucun ignoré : suite complète
  `windows/tests/DmxMoney.Tests`, incluant noyau/interops, modèles de vue, 24 contrôles de
  l'updater et 2 régressions du compagnon distant. Interops et modèles recompilés sur macOS.
- **CI Windows x64 réussie** sur le commit `0f40717` : compilation Rust, bindings C#,
  application WinUI, suite .NET, démarrage et navigation automatique dans toutes les pages.
  Le job publie seulement la version de développement avec `-SkipInstaller` ; il ne produit
  aucune prétendue signature ni installeur officiel. Preuve :
  [job Windows du run 36876164473](https://github.com/thefrcrazy/dmx-money-2/actions/runs/36876164473/job/110416260391).
- Parsing PowerShell des six scripts de distribution/tests et compilation du helper C#
  Authenticode réussis sur macOS. Les 10 tests de refus d'options ont réussi : version
  contenant une injection, signature absente/incomplète, combinaison excluant la vérification
  de l'installeur, empreinte injectée, mélange autosigné/public et endpoint de signature HTTP.
  Ces contrôles portables n'exécutent pas les API cryptographiques Windows.
  Le runtime PowerShell portable officiel
  `7.6.6`, conservé dans `/tmp`, a été vérifié contre le SHA-256 de son asset GitHub.
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

- Exécuter les nouvelles fixtures Authenticode sur Windows réel : certificat non approuvé
  éphémère, signature positive, mauvais certificat, fichier non signé, PE/CMS altérés,
  horodatage RFC 3161 Microsoft réel, jeton altéré et refus de la fixture autosignée en
  mode Public Trust malgré une variable de développement héritée. Ce test est désormais appelé
  automatiquement par `scripts/tests/test-windows-release.ps1` sous Windows ; il supprime
  ensuite le certificat et sa clé. À la rédaction de cette entrée, ces nouvelles API n'ont
  pas encore été exécutées depuis le Mac : elles ne sont pas annoncées comme validées.
- Configurer réellement le compte Azure, la validation d'identité, le profil Public Trust,
  le rôle de signature et les variables GitHub décrits dans `docs/release.md`.
- Exécuter le pipeline Windows, vérifier les signatures réelles et essayer installation,
  désinstallation et mise à jour x64/arm64 sous Windows 11 avec Smart App Control actif.
- Compiler/exécuter GTK et valider Windows arm64 sur leurs systèmes. La CI Windows x64
  réussie confirme compilation et démarrage des pages, mais ne remplace pas une revue visuelle
  ni les essais de synchronisation mobile ou de signature/distribution.

Smart App Control exige une signature publiquement reconnue lorsque la réputation ne suffit
pas. SmartScreen gère séparément sa réputation : une signature valide, y compris EV ou
Artifact Signing, ne garantit plus l'absence immédiate de ses avertissements. Aucun changement
ne désactive les protections Windows. Sources :
[Smart App Control](https://learn.microsoft.com/en-us/windows/apps/develop/smart-app-control/overview),
[réputation SmartScreen](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation),
[intégration Artifact Signing](https://learn.microsoft.com/en-us/azure/artifact-signing/how-to-signing-integrations).
