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
| Windows : 5 000 opérations, 16 lectures sans écriture | 16 remplacements de lignes ; 956,68 ms ; 102,7 Mo alloués | 0 remplacement ; 4,18 ms ; 33,9 Ko alloués |
| GTK : 10 000 binds, 64 emplacements, banc de widgets natifs | 190 000 widgets créés | 1 216 |

La mesure Apple utilise les vrais exports UniFFI de la bibliothèque Rust préexistante du 16 septembre, et le vrai code Swift de cache. Le banc reload Apple extrait le corps réel avec un moteur factice. Windows utilise le noyau réel dans une base en mémoire. Le banc GTK compare les mêmes types de widgets et séquences de binds ; il ne lance pas l'application complète. Les temps ne sont pas extrapolés en gain FPS. Les frames du petit scroll web sont similaires avant/après sur cette machine rapide (p95 11,0 / 10,9 ms) ; la suppression des travaux et la borne du DOM sont les améliorations démontrées.

## Vérifications

- 90 tests PWA, 375 assertions ; typage et build production réussis.
- 3 tests de bornes : début/milieu/fin, en-têtes variables, liste vide/masquée, scroll au-delà de la fin.
- Navigateur réel sur le composant production avec contexte synthétique : recherche unique, résultats vides, sélection, pointage, Tab/Maj+Tab à une frontière et brouillon desktop conservé lors d'un scroll de 20 000 px.
- Mise à jour simulée au milieu : l'opération `fixture-1305` garde son top à −20,5 px après insertion puis suppression au-dessus.
- 60 tests Windows, dont lecture sans rebind, conservation de sélection après écriture, passage de minuit et réglages asynchrones.
- GTK `cargo check` réussi. Les grandes cibles locales n'ont pas été recréées ; le cache temporaire sera supprimé après contrôle.
- 5 nouveaux XCTest Apple ; validation syntaxique et fixtures légères réussies. Exécution complète et builds multiplateformes à confirmer par la CI avant publication.
- 6 tests de politique de préversions et cohérence croisée des versions réussis.

Banc web reproductible : [README](../../pwa/scripts/scroll-perf/README.md). Le banc ne fait pas partie des routes déployées.
