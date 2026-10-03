# Défilement des opérations — 2.1.0 RC2

## Modifications

- PWA mobile : fenêtre virtuelle commune aux opérations et en-têtes de date, recherche binaire des bornes, buffer de 400 px et mises à jour par animation frame. Une journée très chargée ne monte pas toutes ses lignes. Les filtres repartent au début ; les mises à jour de données gardent l'ancre visible. Les contrôles focalisés restent montés et Tab/Maj+Tab peuvent traverser la fenêtre.
- Tableau web : passage d'une mise à jour par pixel à une mise à jour par ligne visible ; hauteur d'en-tête prise en compte. Les infobulles natives remplacent les mesures de troncature et timers par cellule. L'éditeur actif reste monté hors écran pour conserver le brouillon.
- Apple : formats exacts du noyau en cache borné (4 352 chaînes maximum), sans changer l'arrondi. Pas de reconstruction des pages après lecture sans changement de version/jour. Pas de parcours de toutes les lignes si la sélection est vide.
- Windows : les lectures asynchrones sans changement de version/jour ne remplacent pas le journal ; le statut du compagnon est toujours publié.
- GTK : cellules et handlers créés lors du setup puis réutilisés au bind. Les boutons prennent l'identifiant de la cellule actuelle ; les éditeurs sont annulés avant recyclage.

Les calculs financiers et les protocoles réseau ne changent pas. RC2 reste une préversion ; la stable 2.0.9 conserve son canal.

## Mesures locales

Banc synthétique, Mac arm64 ; aucune donnée de l'utilisateur modifiée. Les résultats ci-dessous mesurent des travaux précis, **pas les FPS d'une session utilisateur**.

| Scénario identique | Avant | Après |
|---|---:|---:|
| PWA production, 30 000 opérations, 390×844, 40 scrolls vers le bas | 3 280 opérations conservées, 45 628 nœuds | 13–17 opérations montées, 1 121 nœuds à la fin ; dernière opération accessible |
| Tableau production 1440×900, 90 frames, 178 px | 5 220 lectures `scrollWidth` | 0 |
| Apple : 60 lignes × 1 000 réaffichages de formats identiques | 240 000 appels noyau ; 377,89 ms | 141 appels ; 18,74 ms |
| Apple : 100 reloads sans écriture, moteur de fixture | 100 reconstructions | 0 |
| Apple : sélection vide, 100 000 opérations | 100 000 identifiants parcourus | 0 |
| Windows : 5 000 opérations, 16 lectures sans écriture, médiane de 3 essais | 16 remplacements de lignes ; 948,18 ms ; 102,7 Mo alloués | 0 remplacement ; 3,56 ms ; 27,7 Ko alloués |
| GTK : 10 000 binds, 64 emplacements, banc de widgets natifs, médiane de 3 essais | 190 000 widgets créés ; 6 414,49 ms | 1 216 widgets ; 204,01 ms |

La mesure Apple utilise les vrais exports UniFFI de la bibliothèque Rust préexistante du 16 septembre, et le vrai code Swift de cache. Le banc reload Apple extrait le corps réel avec un moteur factice. Windows utilise le noyau réel dans une base en mémoire. Le banc GTK compare les mêmes types de widgets et séquences de binds ; il ne lance pas l'application complète. Les temps ne sont pas extrapolés en gain FPS. Les frames du petit scroll web sont similaires avant/après sur cette machine rapide (p95 11,0 / 10,9 ms) ; la suppression des travaux et la borne du DOM sont les améliorations démontrées.

## Vérifications

- 90 tests PWA, 375 assertions ; typage et build production réussis.
- 3 tests de bornes : début/milieu/fin, en-têtes variables, liste vide/masquée, scroll au-delà de la fin.
- Navigateur réel sur le composant production avec contexte synthétique : recherche unique, résultats vides, sélection, pointage, Tab/Maj+Tab à une frontière et brouillon desktop conservé lors d'un scroll de 20 000 px.
- Mise à jour simulée au milieu : l'opération `fixture-1305` garde son top à −20,5 px après insertion puis suppression au-dessus.
- 60 tests Windows, dont lecture sans rebind, conservation de sélection après écriture, passage de minuit et réglages asynchrones.
- GTK `cargo check` local et contrôles CI clippy, build et métadonnées d'installation réussis. Les grandes cibles locales n'ont pas été recréées ; les caches temporaires des bancs ont été supprimés.
- 18 XCTest Apple réussis, dont 5 nouveaux tests de performance fonctionnelle ; builds SwiftUI moderne/ancien, AppKit et iOS réussis en CI.
- 6 tests de politique de préversions et cohérence croisée des versions réussis.
- CI de la [PR #5](https://github.com/thefrcrazy/dmx-money-2/pull/5), [run 37111149213](https://github.com/thefrcrazy/dmx-money-2/actions/runs/37111149213), puis CI main [37111768517](https://github.com/thefrcrazy/dmx-money-2/actions/runs/37111768517) entièrement réussies. La fusion `07a54107f847f5d9fb127d709004a8d7665e73cb` conserve exactement l'arbre testé de la PR.

## Publication

- [Workflow de publication 37111769356](https://github.com/thefrcrazy/dmx-money-2/actions/runs/37111769356) réussi pour Windows x64/ARM64, macOS Apple Silicon/Intel, Linux AppImage et Flatpak.
- [2.1.0 RC2](https://github.com/thefrcrazy/dmx-money-2/releases/tag/v2.1.0-rc.2) publiée en préversion depuis `07a54107f847f5d9fb127d709004a8d7665e73cb` ; 23 assets uniques, non vides, avec SHA-256. `latest` reste 2.0.9 et le flux macOS stable conserve son SHA-256 `58dcd6a35ead63fb76c0ab1e1f1909eece7d129248a4747a24768c98544b78c3`.
- PWA [Cloudflare Pages](https://dmxmoney-companion.pages.dev/mobile/) déployée depuis la même fusion : déploiement `1fcdbc63-0cb0-47bb-931a-bb711db7d7f9`, cache `2.1.0-rc.2-992498bd975c2517`. Les 22 fichiers publics ont été téléchargés et leurs SHA-256 correspondent au build local. Le relais et les appairages ne changent pas.
- Windows : signatures du Setup et des binaires vérifiées dans les deux jobs ; les deux certificats X.509 publics conservent l'identité `CN=Collignon Maxim,O=Developmax` et le SHA-256 `2DBBF24B7D4524A6724889CC58DF3C6B90976FA3E26705F8D0DAEC85A0C3DA6B` de RC1. Flux x64/ARM64 et `updates-rc.json` vérifiés en 2.1.0-rc.2, tailles et digests conformes aux paquets GitHub. Aucun PFX ni clé privée publié. La signature reste autosignée et ne garantit pas l'acceptation par Smart App Control.
- DMG Apple Silicon et Intel téléchargés : SHA-256 conformes à GitHub, signatures adhoc `codesign --verify --deep --strict` valides, bundle `com.dmxmoney.app`, architectures arm64/x86_64 et version 2.1.0/build 8. Le noyau RC2 est confirmé ; les DMG n'embarquent pas de PWA. Le flux RC2 contient les deux bons DMG et leurs versions minimales macOS.
- Linux : AppImage ELF64 x86_64/SquashFS 4.0 et métadonnées Flatpak contrôlés après téléchargement ; aucune exécution Linux sur ce Mac. Les paquets Windows ont été vérifiés sur les runners Windows, sans téléchargement des grandes archives sur ce Mac.
- Application Mac installée depuis le DMG ARM validé et relancée, sans intervention sur la base financière ni le trousseau. Contrôle natif : pied de page `DmxMoney • v2.1.0-rc.2`, À propos `Version 2.1.0 (8)`. Le compagnon est disponible, chiffrement et relais sécurisé prêts, connexion Internet connectée ; aucune interface de migration. Aucun QR, appairage ou préférence modifié pendant cette vérification.

Banc web reproductible : [README](../../pwa/scripts/scroll-perf/README.md). Le banc ne fait pas partie des routes déployées.
