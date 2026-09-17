# Synchronisation du compagnon mobile

Le Mac/PC héberge la base de référence. La PWA conserve la dernière copie reçue et une
file persistante de modifications. Hors du réseau local, les pages utilisent cette copie ;
les échéances affichées ne deviennent des écritures définitives que lorsque le moteur Rust
peut les traiter. Les analyses et les soldes utilisent les opérations de cette même copie.

## Garanties et arbitrages

- Chaque création conserve son identifiant pendant tous les renvois. Deux dépenses distinctes
  ne sont pas fusionnées simplement parce que leurs dates, descriptions et montants sont égaux.
- Une occurrence d'échéance est identifiée par l'échéance, sa date et sa contrepartie éventuelle.
  La génération et l'avancement sont transactionnels. Un traitement simultané ou répété ne
  crée pas une seconde occurrence. Les occurrences supprimées ne sont pas recréées.
- Le cache local et le message à envoyer sont écrits dans une seule transaction IndexedDB.
  Une erreur d'enregistrement annule les deux. Les écritures concurrentes sont sérialisées.
- Les nouvelles modifications de compte, opération, catégorie, budget et échéance comportent
  leur version de départ. Seuls les champs effectivement modifiés sont appliqués : pointer
  une opération ne rétablit pas un ancien montant, modifier un libellé d'échéance ne recule
  pas la prochaine date calculée sur le Mac.
- Si le même champ a été modifié des deux côtés, la dernière modification appliquée gagne.
  La suppression d'un enregistrement prime sur une modification devenue obsolète.
- Le serveur enregistre les modifications partielles et leur reçu dans une seule transaction.
  Un accusé de réception perdu ne provoque pas une seconde application, même après une
  modification ultérieure sur le Mac. Les nouveaux virements utilisent aussi ces reçus.
- Le mobile retire un message seulement après une réponse positive. Les erreurs laissent la
  file intacte et affichent un état de synchronisation en attente, sans masquer les données.
- Une réponse de lecture ancienne ne peut pas écraser une nouvelle saisie, même si cette
  saisie a déjà été acquittée pendant que la lecture était en vol.

Les modifications déjà mises en file par une ancienne PWA conservent leur protocole d'origine :
il n'est pas possible de reconstruire rétroactivement une version de départ absente. Installer
la nouvelle version sur l'ordinateur puis charger la PWA connectée avant les nouvelles saisies.
Le mode hors ligne ne permet pas de recevoir les changements du Mac en 4G : la synchronisation
reprend quand le pont est accessible et que la PWA s'exécute (notamment à sa réouverture).

## Vérifications

Les tests HTTP du pont couvrent le pointage avec montant modifié sur le Mac, les renvois,
les suppressions, les virements et les échéances uniques/concurrentes. Les tests PWA couvrent
chaque collection, les écritures IndexedDB concurrentes, l'annulation en cas d'erreur,
l'ordre des messages et la protection contre une réponse réseau ancienne.
