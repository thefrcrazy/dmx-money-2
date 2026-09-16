# Audit transversal — 16 septembre 2026

## Périmètre et statut

Revue des parcours d’écriture et de calcul Rust, façade FFI, API HTTP compagnon, authentification/session/passkey et service Cloudflare, vues Swift/AppKit et transmission du thème, statistiques et mutations hors ligne PWA, modèles Windows, dépendances et scripts de génération/CI. Les fichiers générés et ressources ne sont pas assimilés à du code relu ligne par ligne. Cette revue et ses tests ne constituent pas une garantie d’absence de tout défaut.

**Ces corrections sont dans le workspace : pas de nouveau tag, de release ou de déploiement au cours de cet audit. La RC2 déjà publiée ne les contient pas.**

## Problèmes corrigés

| Zone | Défaut | Correction et vérification |
|---|---|---|
| AppKit / SwiftUI | Thème forcé non transmis explicitement aux vues hébergées conservées ; sélection dépendante de la vibrance et couleurs de calque figées | Injection du thème dans `StoreRoot`, sélection de barre latérale dessinée avec un fond d’accent translucide, couleurs adaptées et réactualisation après changement d’apparence. Captures en clair/sombre puis retour au clair. |
| Icônes de comptes/catégories | Couleurs très pâles sur pastilles claires, et glyphes toujours blancs dans les pastilles pleines | Ajustement du glyphe vers une variante de même teinte, contraste cible de 3:1 avec le fond estimé ; choix noir/blanc pour les pastilles pleines. Couleur stockée et fond conservés. Tests sur blanc, noir, vert pâle et bleu dans les deux thèmes. |
| Générateur Apple | La régénération Lucide supprimait aussi `MenuBarIcon`, ressource maintenue séparément | Conservation explicite de cette ressource pendant la régénération. |
| Core / analyses PWA | Les contreparties de virements gonflaient revenus, dépenses, catégories et graphiques | Exclusion des virements internes de ces statistiques. Soldes et historique des comptes continuent à inclure les flux. Test avec tous les comptes et avec le compte source seul. |
| API compagnon / PWA | Création de virements déséquilibrés possible ; édition d’une seule contrepartie ; liaison modifiable | Validation des deux côtés, écriture atomique, contrôle de la liaison réciproque, protection des identifiants déjà utilisés. Montant/date/libellé/pointage propagés à la contrepartie dans le serveur, l’interface et le cache hors ligne. Rejeu identique accepté ; collision différente refusée. |
| Validation monétaire | Arrondi vers zéro accepté pour un montant positif ; débordement après multiplication ; imports contournant les formulaires | Refus après arrondi, plage des centimes entiers exactement représentables par les clients JavaScript, contrôle des écritures bas niveau et de l’import complet avant effacement. Test de restauration invalide préservant les données existantes. |
| HTTP Rust | `Content-Length` invalide traité comme zéro, taille gigantesque susceptible de déborder, corps tronqué accepté, nombre de threads non borné | Longueur stricte et bornée ; refus des doublons, de `Transfer-Encoding` non pris en charge, des corps incomplets et des en-têtes >32 Kio. Maximum de 64 connexions traitées simultanément, libération automatique des places. Tests de cadrage HTTP et de capacité. |
| Cloudflare | Le secret d’inscription partagé autorisait la rotation d’un appareil existant | Un appareil existant doit prouver la possession de son propre secret. Tests : refus avec secret partagé, aucune écriture, rotation légitime conservée. |
| Cloudflare | Taille JSON limitée uniquement par un en-tête déclaratif, typage TypeScript sans validation runtime | Lecture du flux bornée à 32 Kio réellement reçus, réponses 400/413, refus des objets/valeurs inattendus ; maintien des champs optionnels `null` du client Rust. |
| Dépendances | Avis de sécurité PWA et `rustls 0.23.44` | Mise à jour dans les plages semver existantes de la PWA ; `rustls >=0.23.45` ; remplacement de `rustls-pemfile` abandonné par le lecteur PEM de `rustls-pki-types`. |
| CI | Tests PWA/Worker absents | Ajout d’un job install verrouillée, typage, tests et build PWA, puis tests Worker. |

## Résultats

- **Rust : 111 tests réussis**, `cargo fmt --all --check` et Clippy sans avertissements sur core, bridge et FFI.
- **PWA : 32 tests réussis**, TypeScript et build production réussis. `bun audit` ne remonte plus d’avis après mise à jour.
- **Worker : 5 tests réussis**, typage du code de production réussi. Tests entièrement locaux avec faux stockage et secrets fictifs.
- **Swift : 9 tests réussis**, dont le contraste des pastilles. Builds AppKit et Apple Silicon réussis avec Xcode 27. Les captures utilisent une base de démonstration isolée.
- **Windows : 13 tests de modèles réussis**. Audit NuGet transitif lors de la restauration WinUI et audit des modèles : aucun avertissement de vulnérabilité. La commande de listing WinUI échoue sur ce Mac (« Sequence contains no matching element ») ; la restauration avec `NuGetAuditMode=all` a été utilisée.
- **RustSec : aucune entrée de la catégorie `vulnerabilities` restante**. Avertissements conservés : `atty` (lecture potentiellement non alignée et abandon), `ansi_term` (abandon), via `ksni → dbus-codegen → clap 2`, chaîne de génération Linux.
- Vérification des icônes natives générées et `git diff --check` réussis.

## Limites et suites recommandées

- Pas de macOS Catalina réel disponible. Xcode 27 refuse la cible 10.15 : le build de capture de la même interface AppKit utilise temporairement la cible 12.0 en ligne de commande ; **la cible 10.15 du projet reste intacte**. Les captures sur macOS récent ne prouvent pas le rendu exact du moteur SVG de Catalina. La compilation de distribution Catalina doit rester réalisée avec Xcode 26 compatible.
- Les builds de capture Swift lient le XCFramework local existant : ils vérifient le rendu. Les nouvelles corrections Rust ont été testées séparément ; reconstruire le XCFramework et les binaires de distribution pour la prochaine release.
- Windows WinUI complet et Linux GTK n’ont pas été exécutés sur leurs OS pendant cette passe ; leurs modèles/contrats partagés et dépendances ont été vérifiés. Le nouveau job CI n’a pas encore été exécuté à distance.
- Migrer ultérieurement la chaîne de génération de l’icône de zone de notification Linux pour retirer `atty`/`ansi_term` ; ne pas confondre ces avertissements avec des vulnérabilités disparues.
- La limitation d’inscriptions Cloudflare actuelle repose sur KV et reste approximative en concurrence. Une limitation strictement atomique nécessiterait un mécanisme dédié. Le secret propre à chaque appareil reste obligatoire.
- Les montants persistés conservent le format flottant compatible 1.x ; les calculs utilisent les centimes. Une migration du format d’échange/stockage doit être traitée séparément avec compatibilité des sauvegardes.
- Aucun test de reconnaissance vocale réelle ou de Siri signé supplémentaire dans cet audit. Aucune base personnelle n’a été modifiée.

## Captures

- [Catégories après sombre → clair](audit-2026-09-16/categories-light.png)
- [Vue d’ensemble sombre](audit-2026-09-16/dashboard-dark.png)

## Sources de référence

- [Apple : teinte des images modèles AppKit](https://developer.apple.com/documentation/AppKit/NSImageView/contentTintColor)
- [RFC 9112 : cadrage HTTP/1.1](https://www.rfc-editor.org/rfc/rfc9112.html)
- [RustSec : correctif TLS rustls](https://rustsec.org/advisories/RUSTSEC-2026-0285.html)
- [Cloudflare : recommandations Workers](https://developers.cloudflare.com/workers/best-practices/workers-best-practices/)
