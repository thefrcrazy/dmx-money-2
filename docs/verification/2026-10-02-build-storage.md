# Espace disque des builds — 2 octobre 2026

Mesure en lecture seule du dépôt local `dmx-money-2`. Aucun cache, historique Git,
code, donnée financière ou sauvegarde n'a été supprimé.

## Les 57,3 Go

L'addition des blocs alloués pour **chaque chemin** donne **57 351 524 352 octets**
(57,35 Go), ce qui explique les 57,3 Go observés dans Finder. Plusieurs chemins
désignent pourtant le même fichier par des liens physiques : après déduplication
par périphérique et inode, l'allocation déclarée est de **48 002 650 112 octets**
(48,00 Go). Cela ne mesure pas les snapshots ni les blocs partagés par les clones APFS.

La taille logique des fichiers, avec chaque chemin compté, est de
56 766 796 567 octets. Les liens physiques dupliquent dans ce total
9 204 431 304 octets logiques. Il ne s'agit pas d'une base financière de cette taille.

| Ensemble | Allocation en Go décimaux, liens physiques dédupliqués |
| --- | ---: |
| `target/` au total | 45,35 |
| dont `target/debug/` | 27,22 |
| dont les quatre cibles Rust Apple, debug et release | 7,23 |
| dont les builds Xcode et audits distincts | 8,77 |
| dont `target/release/` | 1,94 |
| cache Swift Package Manager dans `apple/Packages/DmxKit/.build/` | 1,03 |
| dépendances du Worker | 0,64 |
| dépendances PWA | 0,40 |
| ensemble Windows, sources et sorties comprises | 0,32 |

Les sous-ensembles `target` sont inclus dans sa ligne totale. Le dépôt V1
`Dmx-Money`, inspecté sans modification, occupe seulement environ **25 Mo** alloués.

## Pourquoi ces caches ont grossi

Les audits précédents ont conservé plusieurs dossiers DerivedData séparés :
`apple-build`, `apple-audit-*`, `apple-review-final`, `xcode-dmxkit*` et `journal-*`.
Leur accumulation explique environ **8,77 Go**. Les bibliothèques FFI statiques
de développement répétées dans ces sorties pèsent jusqu'à 578 Mo chacune.

Les compilations Rust conservent aussi plusieurs variantes de dépendances et de
tests, selon la cible, le profil, les features et les options de compilation.
`target/debug/deps` contient environ 14,23 Go alloués et plus de 82 000 fichiers
objets `.o`. Cargo active par défaut les informations de débogage complètes et
la compilation incrémentale en développement ; celle-ci conserve des données
supplémentaires pour accélérer la recompilation.
Voir les [profils Cargo](https://doc.rust-lang.org/cargo/reference/profiles.html)
et la [structure du cache](https://doc.rust-lang.org/cargo/reference/build-cache.html).

## Nettoyage préparé, non exécuté

Depuis la racine du dépôt :

```bash
bash scripts/clean-build-cache.sh
bash scripts/clean-build-cache.sh --scope audits
```

Ces commandes simulent seulement. Après arrêt des compilations, `--apply` autorise
la suppression des seuls dossiers listés. Le script refuse les compilations actives,
les fichiers versionnés, les liens symboliques et les fichiers de données ou secrets
identifiables. Il préserve les bases de démonstration et captures UI placées à côté des
builds, `dist`, les frameworks/bindings générés et les dépendances installées.

Gains estimés hors liens physiques conservés ailleurs : **8,68 Go** pour les caches
incrémentaux et **8,76 Go** pour les anciens builds d'audit, soit environ **17,4 Go**.
Les caches supprimés seront reconstruits ; la compilation suivante prendra plus de temps.
Ce gain reste une estimation avant nettoyage, hors snapshots/clones APFS.

Pour les prochains audits ponctuels, utiliser un dossier de build stable au lieu
de créer un nouveau DerivedData à chaque vérification. `CARGO_INCREMENTAL=0`
évite la constitution de nouveaux caches incrémentaux, au prix de recompilations
plus lentes ; cette option ne nettoie pas les caches existants. Réduire le niveau
de debug à `1` peut aussi économiser de l'espace, mais limite l'inspection des variables
au débogueur. Aucun profil Cargo n'a été modifié sur cette passe.

Vérifications : syntaxe Bash/Python, simulations des deux scopes, refus non destructif
des données, fichiers suivis, symlinks et d'un processus Cargo actif synthétique.
`cargo clean --offline --locked --dry-run --profile dev` a affiché 129 451 fichiers,
33,1 GiB, puis confirmé qu'aucun fichier n'avait été supprimé. Un `cargo clean`
sans sélection supprimerait tout `target` : ce n'est pas le nettoyage retenu ici.
Voir la [commande officielle](https://doc.rust-lang.org/cargo/commands/cargo-clean.html).
