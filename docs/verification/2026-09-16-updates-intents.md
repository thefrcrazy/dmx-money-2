# Vérification des mises à jour et intentions — 16 septembre 2026

## Défauts corrigés

- macOS : remplacement de la boucle modale de téléchargement par une feuille AppKit non bloquante. Le fichier temporaire est déplacé avant le retour du callback URLSession. Erreurs HTTP, échec de déplacement, annulation et limite totale de dix minutes sont traités. Les téléchargements simultanés sont évités.
- Installation macOS : copie complète avant remplacement, conservation de l'ancienne app pendant le remplacement, validation de la signature et de l'identifiant du bundle. Les chemins passent comme arguments du script, sans interpolation shell. Le paquet Apple Silicon requiert bien macOS 26.
- Windows : refus des tentatives simultanées, recherche limitée à 30 secondes, jeton d'annulation du téléchargement après dix minutes, erreurs signalées au lieu de remonter dans le gestionnaire de menu asynchrone.
- Siri : ajout de `TransferMoneyIntent` et exposition de `ProcessDueIntent` dans les raccourcis. Recherche des comptes/catégories via `EntityStringQuery`. Les intentions structurées utilisent les identifiants sélectionnés, plutôt que de reconstruire une phrase avec leurs noms.
- Localisation : phrases, titres et résumés FR/EN dans leurs tables respectives. Retrait des titres courts placés à tort dans `AppShortcuts.xcstrings`.
- Noyau : virements textuels FR/EN, direction explicite dans les deux ordres, noms complets et non ambigus. Une demande de virement incomplète ne devient plus une dépense et ne choisit plus arbitrairement la source. Les demandes inconnues avec un nombre seul ne sont plus enregistrées comme dépenses.

## Vérifications

- Suite Rust `dmx-core` et `dmx-ffi` : succès ; quinze tests de l’assistant réussis, y compris les noms qui se chevauchent ou contiennent des chiffres ; tests de régression supplémentaires dans `core/crates/dmx-core/tests/assistant.rs`.
- Swift/DmxKit : sept tests réussis, dont le virement par identifiants, ses soldes, et le rejet des montants invalides, du compte absent et d'une source identique à la destination.
- Test AppKit isolé : succès du téléchargement, erreur HTTP 404, annulation et callback tardif. Pas d'installation ni d'accès aux données personnelles. Commande : `python3 scripts/tests/test-macos-updater.py`.
- Compilation macOS Intel/Catalina : réussie (code de téléchargement commun).
- Compilation macOS moderne : réussie. Huit intentions dans `Metadata.appintents/extract.actionsdata`, dont `TransferMoneyIntent`. Tables `AppIntents`, `AppShortcuts` et `Localizable` présentes pour `fr` et `en`, avec paramètres de traduction conservés.
- Syntaxe du script d'installation : vérifiée par `/bin/sh -n`. Le remplacement d'une application réelle n'a pas été exécuté.

## Limites et installation actuelle

L'app `/Applications/DmxMoney.app`, version `2.0.3-rc.1`, contient encore les sept anciennes intentions et une signature ad hoc sans Team ID. Les changements de ce travail sont dans le projet et les builds de validation ; l'app installée n'a pas été remplacée. Le trousseau accessible lors du contrôle n'exposait aucune identité de signature valide.

La reconnaissance vocale et l'indexation Siri sur macOS 27 ne sont pas validées par une compilation. Un essai vocal avec une app correctement signée et installée reste nécessaire. Les deux langues déclarées sont le français et l'anglais ; les réponses du noyau restent françaises et son analyse textuelle accepte un ensemble limité de formulations avec montants en chiffres. Aucune garantie de compréhension universelle des langues.

La compilation Windows n'a pas été validée : les dépendances NuGet ne sont pas restaurées dans cet environnement (`NETSDK1004`). Les essais d'installation Velopack nécessitent Windows. Linux ouvre la page des versions et ne possède pas cette fenêtre de téléchargement. Dans un navigateur ordinaire, le hook de mise à jour Tauri de la PWA est inactif. iOS n'inclut pas actuellement le fichier d'intentions propre à la variante macOS moderne.

## Références vérifiées

- [Apple — durée de vie du fichier temporaire téléchargé](https://developer.apple.com/documentation/foundation/downloading-files-from-websites).
- [Apple — résolution d'entités par nom avec EntityStringQuery](https://developer.apple.com/documentation/appintents/entitystringquery).
- [Velopack — UpdateManager et annulation du téléchargement](https://docs.velopack.io/reference/cs/Velopack/UpdateManager).
