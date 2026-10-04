# Défilement et changement de page macOS

4 octobre 2026. Baseline : `decec65429b1ef1b02c2bc53a4efa5d769329e6c`. Seul le dépôt V2 est modifié.

Le journal moderne utilisait une `Table` SwiftUI avec neuf colonnes, des états d'édition par cellule et une préparation des lignes liée aux notifications d'interface. Le défilement rapide dans cette source reproduit un coût élevé de rendu et de retrait de la page dans la fixture AppKit. L'application installée conserve l'ancien build 8 ; ce correctif n'est pas encore publié.

## Corrections

- Journal moderne et legacy : contrôleur `NSTableView` partagé, cellules réutilisables, hauteur fixe et positions calculées sans contraintes AutoLayout par cellule. Les icônes et dimensions de texte sont réutilisées lorsque leur contenu ne change pas. [Réutilisation des vues documentée par Apple](https://developer.apple.com/documentation/appkit/nstableview/makeview(withidentifier:owner:)).
- Tri et index d'IDs préparés sur une file de fond. Une notification de sélection ou de chargement ne reconstruit plus les lignes. Le retrait de la page annule la publication et détache les delegates ; l'édition valide en cours est finalisée avant le retrait.
- Échéancier moderne : lignes triées conservées jusqu'à une nouvelle révision ou un changement de tri ; préparation hors UI et annulée au masquage. Les sélections obsolètes sont retirées.
- Budget moderne : catégories et enveloppes dans des conteneurs paresseux, avec IDs stables.

## Mesures et vérifications

Commande : `python3 scripts/tests/test-macos-journal.py`, macOS Apple Silicon, compilation Swift 5 optimisée avec cible macOS 26. Exécution comparative locale avec 30 000 opérations synthétiques et 80 sauts alternant le début et la fin de la table. Chaque saut appelle `scrollRowToVisible`, force layout/affichage et traite brièvement la boucle d'événements. Les deux variantes sont compilées et lancées dans les mêmes conditions ; la baseline est toujours exécutée en premier. Le retrait est mesuré immédiatement après la même séquence de défilement, sans édition/tri intercalés. Les assertions fonctionnelles utilisent un troisième processus distinct. Les caches de fenêtres/polices et la charge du host peuvent varier.

| Mesure | Avant | Après |
| --- | --- | --- |
| Saut de défilement, médiane | 84,4 ms | 25,8 ms |
| Saut de défilement, 95e percentile | 100,2 ms | 30,9 ms |
| Retrait du journal après les sauts | 129,5 ms | 18,9 ms |

Onze assertions utilisent le contrôleur et les cellules réels : édition du libellé/montant sur l'ID capturé, rejet du brouillon après changement de révision, tri stable, index d'ID, sélection après tri, pointage de la bonne ligne, contenu affiché et détachement des delegates. Les résultats asynchrones sont attendus avec une échéance bornée, plutôt qu'un délai arbitraire ; les échecs indiquent leur ligne. Le test est ajouté à la CI Apple avec conservation des JSON/logs/captures de sa propre fenêtre. Aucun seuil de timing dépendant du host n'est imposé.

Un XCTest vérifie également que la révision de l'échéancier n'avance qu'après une réponse visible, ne change pas pour un filtre sur page masquée et reprend à la réactivation.

## Portée et limites

La fixture compile la source réelle du journal avec des collaborateurs DmxKit synthétiques : aucune FFI, base utilisateur, connexion CloudKit ou synchronisation distante. La destination est un simple texte « Budget fictif » : le temps de retrait ne mesure pas le chargement complet du budget ou de l'échéancier. Les captures proviennent uniquement de sa propre `NSHostingView`, sans capture de l'application installée.

Ces résultats ne mesurent ni gestes physiques, ni FPS, ni performances sur Intel/Catalina. Les changements du budget et de l'échéancier sont vérifiés par compilation et tests de cycle de vie, sans gain chiffré attribué à ces pages. Les builds multiplateformes restent obligatoires avant livraison ; aucun tag ou déploiement de production n'est créé ici.
