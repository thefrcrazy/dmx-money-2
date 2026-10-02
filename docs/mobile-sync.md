# Synchronisation du compagnon mobile

Le Mac/PC héberge la base de référence. Activer le compagnon dans les paramètres puis
scanner son QR ouvre la PWA commune sur Cloudflare Pages et permet de créer une passkey.
Le bureau ouvre une connexion WebSocket sortante au relais Internet. Aucun domaine,
DNS, certificat local ou port entrant n'est à configurer.

La PWA conserve la dernière copie reçue et une
file persistante de modifications. Lorsque le bureau est injoignable, les pages utilisent cette copie ;
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

Le relais Internet permet de synchroniser en Wi-Fi, 4G ou 5G tant que le bureau est
allumé, connecté et DmxMoney ouvert.
La PWA active vérifie les changements toutes les 2,5 secondes. Une PWA suspendue en
arrière-plan par le téléphone reprend à sa réouverture ; elle ne peut pas garantir un
traitement continu en arrière-plan.

Le relais utilise le même moteur et le même protocole de mutations. Les transactions
sont téléchargées par pages de 2 000, avec une version bancaire contrôlée entre les pages
et un remplacement du cache seulement après réception complète. Un changement concurrent
fait recommencer la lecture, au plus deux fois, puis conserve le cache existant.

La clé du relais passe par le fragment du QR et ne va pas au Worker. Les passkeys imposent
une vérification utilisateur (`userVerification=required`) à l'appairage et lorsqu'une
nouvelle authentification est nécessaire. Le système choisit Face ID, Touch ID ou le
code de l'appareil ; Safari ne permet pas d'imposer une méthode biométrique particulière.
La session finalisée est enregistrée chiffrée dans IndexedDB avec une CryptoKey non
extractable. Au lancement et au retour au premier plan, la PWA la reprend silencieusement
auprès du bureau. Sa durée est limitée à sept jours d'inactivité et trente jours depuis
sa création. Une session valide évite de répéter la passkey à chaque réouverture.
Les appairages inachevés et leur QR restent limités à dix minutes.

Révoquer un appareil ou sa passkey invalide immédiatement ses sessions sur le bureau.
« Verrouiller » retire la session locale et impose une nouvelle authentification ;
la clé du relais, les données du cache et la file de modifications restent conservées.
Le verrouillage du téléphone protège également l'accès à une session encore valide.

Le cache financier et les corps des messages hors ligne sont
stockés dans des enveloppes JSON UTF-8/base64 versionnées. Les anciennes valeurs sont
migrées dans une transaction IndexedDB, sans perdre les messages en attente. L’encodage
masque la lecture directe et ajoute environ 33 % à la taille du JSON UTF-8 ; il reste
facilement décodable et n’apporte pas de confidentialité. La garde d’interface masque les
données sans session authentifiée ou après verrouillage, sans chiffrer ces fichiers financiers.
Le chiffrement de la session protège ses identifiants ; il ne chiffre pas le cache financier.
Un opérateur qui contrôlerait le JavaScript livré pourrait compromettre ce client.
Voir [les limites et le déploiement du relais](../cloudflare/remote-relay/README.md).

## Vérifications

Les tests HTTP du pont couvrent le pointage avec montant modifié sur le Mac, les renvois,
les suppressions, les virements et les échéances uniques/concurrentes. Les tests PWA couvrent
chaque collection, les écritures IndexedDB concurrentes, l'annulation en cas d'erreur,
l'ordre des messages et la protection contre une réponse réseau ancienne.
