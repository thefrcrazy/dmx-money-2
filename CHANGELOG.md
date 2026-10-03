# Changelog

## 2.1.0-rc.2 — 2026-10-03

- Journal PWA mobile : virtualisation des opérations et des en-têtes de date avec une fenêtre de rendu bornée, même après un long défilement ou sur une journée très chargée ; conservation de la sélection et de la ligne focalisée.
- Tableaux PWA : mises à jour par ligne visible, infobulles natives sans mesures répétées et conservation du brouillon lorsque la ligne éditée quitte l’écran.
- Journaux natifs SwiftUI/AppKit, WinUI et GTK : réutilisation des formats ou cellules et réduction des rechargements de listes pendant le défilement.
- Les lectures du statut du compagnon et les vérifications d'échéances sans modification ne recalculent plus inutilement le journal ; le changement de jour continue d'actualiser les vues qui en dépendent.
- Métadonnées alignées sur `2.1.0-rc.2`, build Apple 8 ; pré-version distincte de RC1, utilisant le flux `updates-rc.json` et conservant le canal stable séparé.

## 2.1.0-rc.1 — 2026-10-02

- Compagnon Internet unique : activation puis appairage QR/passkey sur la PWA Cloudflare Pages, avec relais chiffré et connexion WebSocket sortante du bureau.
- Suppression du serveur HTTPS local, de la gestion DNS/ACME, de l'ancien Worker et des boutons de migration sur SwiftUI, AppKit, WinUI et GTK.
- Configuration du compagnon simplifiée : aucune adresse de service ni domaine à saisir ; l'identité et les appareils déjà appairés au relais actuel sont conservés.
- Reprise de la session mobile après réouverture et reconnexion, avec identifiants chiffrés dans IndexedDB, expiration après sept jours d'inactivité ou trente jours au total, révocation immédiate des sessions d'un appareil et verrouillage volontaire. La passkey reste requise à l'appairage et après verrouillage ou expiration.
- PWA publiée séparément sur Pages ; suppression de son embarquement dans les paquets macOS et Linux et des anciens secrets partagés de la chaîne de publication.
- Pré-version RC1 publiée sur le canal GitHub des préversions, avec flux macOS `updates-rc.json` séparé ; la version stable reste disponible pour les installations qui n'autorisent pas les préversions.
- Le bureau doit rester allumé, connecté à Internet et DmxMoney ouvert. Le cache financier mobile reste obfusqué en base64, décodable et sans chiffrement au repos ; la session seule est protégée par chiffrement.

## 2.0.9 — 2026-10-02

- PWA hébergée sur Cloudflare Pages ; API de relais séparée sur un Worker et WebSocket sortant depuis le bureau, sans DNS individuel.
- Correction de la migration : une identité DNS héritée ne devient plus un faux accès Internet ; action explicite de passage au relais sur SwiftUI, AppKit, WinUI et GTK.
- Nouvel appairage sur Pages : anciennes passkeys, sessions et QR révoqués lorsque l’origine change ; l’ancien QR en mémoire n’est plus affiché après activation.
- Une activation Internet échouée conserve l’ancienne configuration et ne se rabat pas sur un pont DNS local. Le démarrage conserve les appairages existants jusqu’à une migration explicite.
- Vérification d’écriture distante : création, modification et suppression reçues par le bureau, contrôle de son snapshot et notifications aux interfaces natives ; autorisation, CSRF et rejeu vérifiés.
- Diagnostic des caches de compilation et outil de nettoyage en simulation par défaut ; aucune donnée financière ni sauvegarde supprimée.

## 2.0.8 — 2026-10-02

- Compagnon Internet : relais Cloudflare commun, connexion WSS sortante du bureau, messages chiffrés AES-GCM et appairage par QR/passkey sans configuration DNS individuelle.
- Journal : affichage mobile progressif, calculs et mises à jour de listes natives optimisés sur SwiftUI/AppKit, WinUI et GTK.
- Imports et restauration : validation des dates et montants avant écriture, transactions atomiques et conservation des dépenses distinctes identiques.
- Synchronisation : protection des accusés de réception CloudKit, changement de compte iCloud, cohérence des snapshots et validation des mutations distantes.
- Distribution Windows : mode autosigné explicitement choisi pour cette version ; contrôle des signatures, exécutables/DLL et paquets, installation des mises à jour après confirmation. Le mode Public Trust reste bloqué si son identité publique n'est pas configurée.
- Builds Windows de développement : signature autosignée locale avec certificat RSA et clé privée non exportable ; diagnostic explicite quand Windows bloque le lancement de l’updater. Le mode administrateur ne contourne pas Smart App Control.
- Updater Windows : contrôle final taille/SHA-256 du paquet complet, y compris s'il est déjà en cache ; refus d'un fichier altéré et verrou de lecture jusqu'au lancement de l'installation.
- Cache mobile : valeurs financières et corps des messages hors ligne obfusqués en UTF-8/base64 ; migration atomique sans suppression des saisies en attente. Cet encodage reste décodable et ne constitue pas un chiffrement.
- Les installeurs Windows de cette version utilisent un certificat autosigné Developmax / Collignon Maxim. Ils ne garantissent pas l'acceptation par Smart App Control ; le mode administrateur ne contourne pas ce blocage.
- Flatpak : runtime GNOME maintenu ; remplacement de GNOME 47 hors maintenance.

## 2.0.7 - 2026-10-02 (PWA ; publication bureau annulée)

- Compagnon Internet : relais Cloudflare commun, connexion WSS sortante du bureau, messages chiffrés AES-GCM et appairage par QR/passkey sans configuration DNS individuelle.
- Journal : affichage mobile progressif, calculs et mises à jour de listes natives optimisés sur SwiftUI/AppKit, WinUI et GTK.
- Imports et restauration : validation des dates et montants avant écriture, transactions atomiques et conservation des dépenses distinctes identiques.
- Synchronisation : protection des accusés de réception CloudKit, changement de compte iCloud, cohérence des snapshots et validation des mutations distantes.
- Distribution Windows : mode autosigné explicitement choisi pour cette version ; contrôle des signatures, exécutables/DLL et paquets, installation des mises à jour après confirmation. Le mode Public Trust reste bloqué si son identité publique n'est pas configurée.
- Builds Windows de développement : signature autosignée locale avec certificat RSA et clé privée non exportable ; diagnostic explicite quand Windows bloque le lancement de l’updater. Le mode administrateur ne contourne pas Smart App Control.
- Updater Windows : contrôle final taille/SHA-256 du paquet complet, y compris s'il est déjà en cache ; refus d'un fichier altéré et verrou de lecture jusqu'au lancement de l'installation.
- Cache mobile : valeurs financières et corps des messages hors ligne obfusqués en UTF-8/base64 ; migration atomique sans suppression des saisies en attente. Cet encodage reste décodable et ne constitue pas un chiffrement.
- Publication bureau annulée avant diffusion des installeurs pour remplacer le runtime Flatpak hors maintenance ; ces changements sont livrés en 2.0.8.

## 2.0.6 - 2026-09-17

- Synchronisation hors ligne : cache et messages sauvegardés atomiquement ; protection contre les saisies concurrentes et les réponses réseau anciennes.
- Modifications partielles des comptes, opérations, catégories, budgets et échéances : pointer une opération ne rétablit plus son ancien montant et éditer une échéance ne recule plus sa prochaine date.
- Accusés de réception persistants pour les modifications partielles et les virements : un renvoi après coupure ne réapplique pas une écriture déjà reçue.
- Échéances traitées sous verrou transactionnel, occurrences supprimées non recréées et créations retardées respectant les suppressions connues.
- État de synchronisation en attente visible sans masquer les données locales.

## 2.0.5 - 2026-09-16

- PWA : ouverture depuis les données enregistrées même lorsque le pont local est inaccessible hors Wi-Fi.
- Saisies hors ligne conservées, envoyées avant le rechargement des données à la reconnexion ; première reconnexion et réveil de l’application corrigés.
- Les erreurs temporaires du pont ne bloquent plus la consultation des données enregistrées.
- Cache de l’interface renouvelé à chaque build et délai réseau limité pour ouvrir la PWA hors ligne, sans effacer les données ni les modifications en attente.

## 2.0.4 - 2026-09-16

- macOS Intel/Catalina : quatre onglets dans les paramètres (Général, Compagnon mobile, Données, À propos), comme dans la variante moderne.
- Action « Vérifier les mises à jour… » rétablie dans les paramètres Intel, avec les mêmes actions partagées que sur Apple Silicon.
- Icônes de repli teintées par leur masque et sélection de navigation lisible en thème clair ; conservation des composants et couleurs v2.
- Vérification CI du rendu AppKit avec les icônes Catalina forcées, passage clair/sombre/clair et parcours des quatre onglets. Ce contrôle ne remplace pas un essai sur macOS 10.15 réel.
- PWA : retraits rouges et signés, montants au format français en euros, titre fixe avec connexion à gauche et assistant à droite.


## 2.0.3 - 2026-09-16

- Version stable intégrant les correctifs des RC : échéances actualisées, soldes persistants, assistant PWA unique et téléchargement des mises à jour fiabilisé.
- Contraste des icônes et de la navigation macOS corrigé en thème clair et lors des changements de thème.
- Virements internes exclus des revenus et dépenses dans les analyses ; modification et synchronisation des deux écritures cohérentes et atomiques.
- Validation renforcée des montants et des sauvegardes avant écriture, encadrement des requêtes HTTP et protection des identifiants des appareils du pont Cloudflare.
- Correctif de sécurité TLS intégré, dépendances PWA actualisées et tests PWA/Worker ajoutés à la CI.


## 2.0.3-rc.2 - 2026-09-16

### Échéances et rafraîchissement

- Vérification des échéances au changement de page, au retour au premier plan et régulièrement lorsque l’application reste ouverte. Les données et les vues dépendantes du jour sont recalculées.
- Génération des échéances de la PWA compagnon par le noyau du desktop, avec déduplication des occurrences.
- Soldes Pointé et Actuel visibles sur toutes les pages des interfaces de bureau, notamment macOS Intel/Catalina.
- Analyse et prédictions de la PWA actualisées au changement de jour ; graphique mensuel de l’accueil limité à la bonne année et hors virements internes.

### Assistant et mises à jour

- Suppression du deuxième assistant intégré à l’accueil PWA ; conservation du bouton global.
- Transcription complète conservée entre les événements vocaux, annulation propre à la fermeture, erreurs de microphone et réseau explicites. La demande dictée reste modifiable avant l’envoi.
- Intent Siri de virement avec comptes source/destination explicites, recherche des comptes et catégories par nom, identifiants structurés et ressources FR/EN corrigées.
- Téléchargement macOS non bloquant, annulation, délais maximaux et gestion des erreurs ; remplacement de l’application avec copie de secours. Délais et erreurs également gérés sous Windows.


## 2.0.3-rc.1 - 2026-09-16

### Mises à jour & Synchronisation des versions

- Alignement des versions Application et Noyau : `AppInfo.version` prend systématiquement sa source de vérité depuis le noyau DmxCore, éliminant tout décalage entre la version affichée et la version réelle du moteur.
- Détection des pré-releases fiabilisée : le flux de mise à jour macOS interroge directement l'API des releases GitHub pour prendre en compte immédiatement toutes les pré-releases publiées.
- Transmission de la version complète à la chaîne de compilation Xcode (`MARKETING_VERSION`).

## 2.0.2-rc.4 - 2026-09-16

### Interface & Ergonomie

- Épuration de la barre latérale : suppression du bouton d'action dans le pied de navigation pour une interface épurée et sans encombrement.
- Libellé officiel macOS : adoption du libellé standard « Vérifier les mises à jour… » dans le menu de l'application et la barre des menus.
- Vérification périodique des mises à jour : contrôle silencieux et automatique toutes les 24 heures (au démarrage et en tâche de fond).

## 2.0.2-rc.3 - 2026-09-16

### Mises à jour & Système

- Mise à jour automatique in-place avec redémarrage (Update & Restart) : téléchargement silencieux en tâche de fond et relance automatique de l'application mise à jour sans manipulation manuelle sur macOS et Windows.
- Gestion complète des pré-releases (SemVer) : case à cocher pour autoriser les pré-releases (bêta / RC) dans les paramètres et détection fluide des montées de version.
- Bouton « Vérifier les mises à jour » multiplateforme : accessible dans la barre latérale et les menus système sous macOS, Windows et Linux.
- Icône monochrome officielle macOS : nouveau pictogramme conforme aux HIG Apple pour la barre de menus supérieure.
- Compagnon mobile & PWA : synchronisation et purge de cache v37 immédiate sur Cloudflare KV.

## 2.0.2-rc.2 - 2026-09-16

### Compagnon mobile & PWA

- Assistant vocal universel : accès direct à l'assistant depuis n'importe quel écran grâce à un bouton dédié sous le badge de synchronisation, avec feuille modale iOS, suggestions rapides et dictée vocale au microphone (compatible Safari iOS).
- Micro dans l'assistant d'accueil : le widget de la vue d'ensemble intègre désormais un bouton dictée pour dicter et exécuter directement ses requêtes.
- Cycle de vie PWA et cache : stratégie de navigation Network-First, mise à jour immédiate du Service Worker et rechargement automatique lors de l'activation d'une nouvelle version.
- Notes de version : mémorisation locale définitive de la consultation des notes pour éviter leur réaffichage à chaque rafraîchissement.

## 2.0.2-rc.1 - 2026-09-15

### Interface

- Icônes natives sur chaque système : SF Symbols sur macOS et iOS, Segoe Fluent Icons sous Windows (sous Windows 10, un glyphe équivalent remplace les rares icônes absentes de sa police Segoe MDL2 Assets), icônes symboliques GNOME (thème Adwaita et GNOME Icon Development Kit) sous Linux. Les dessins Lucide ne servent plus qu'à la PWA et à macOS 10.15, qui n'a pas de SF Symbols.
- PWA du compagnon mobile : design d'app iOS en thème clair et sombre (grands titres, barre d'onglets flottante, listes groupées, feuilles qui montent du bas, champs et boutons système). Le Journal devient une liste par jour : toucher une ligne l'ouvre, le rond la pointe, « Sélectionner » regroupe les actions.

### Corrections

- Compagnon mobile : l'appairage échouait quand la PWA était servie par le Mac, et l'app affichait « aucun mobile appairé ». Le pont n'acceptait que les clés d'accès créées depuis l'adresse publique de la PWA.

- Windows : l'app se fermait en ouvrant la page Analyses, restée à moitié affichée (plage vide, graphiques non remplis).
- Windows : une page qui ne peut pas s'ouvrir laisse désormais son exception réelle dans `crash.log`, avec les dernières étapes (pages, formulaires) ; la CI ouvre chaque page de l'app publiée.

## 2.0.1 - 2026-09-14

### Corrections

- Windows : l'application se fermait dès son lancement, sans fenêtre ni message, avec l'installeur comme avec la version portable. Le fichier `resources.pri`, qui contient le XAML compilé de l'app, manquait à la publication.
- Windows : une erreur fatale affiche désormais une boîte de dialogue et laisse son détail dans `%LOCALAPPDATA%\DmxMoney\crash.log`.

## 2.0.0 - 2026-09-14

### Applications natives

- Réécriture complète de DmxMoney en applications natives : AppKit + SwiftUI sur macOS, SwiftUI sur iOS et iPadOS, WinUI 3 sur Windows, GTK4 + libadwaita sur Linux.
- Noyau Rust partagé (`dmx-core`) pour la base SQLite, les règles métier, les imports CSV/QIF/OFX, les sauvegardes `.dmx`, la synchronisation et le pont compagnon mobile. Les interfaces n'effectuent aucun calcul : les mêmes montants sont affichés sur les trois systèmes.
- Parité fonctionnelle avec la 1.x sur les neuf écrans : vue d'ensemble, comptes, journal, budget, échéancier, analyses, prédictions, catégories, paramètres.
- Assistant du noyau : « ajoute 12,50 € en alimentation », « quel est mon solde ? », « combien me reste-t-il en carburant ? », « prochaines échéances », « résumé du mois », « traiter les échéances dues ». Analyse déterministe et testée, montants calculés par `dmx-core`.
- Virements : pointer une jambe pointe aussi la jambe liée ; les montants affichent « − » en sortie et « + » en entrée, en gardant leur couleur.
- Intentions Siri et Raccourcis : comptes et catégories proposés en liste, réponse visible (vue, valeur renvoyée, notification), titres et descriptions traduits en anglais.
- macOS : sept intentions Siri et Raccourcis (App Intents) branchées sur l'assistant du noyau, phrases en français et en anglais, app signée avec ressources scellées pour que Raccourcis puisse la joindre.
- Compagnon mobile : `POST /api/assistant` sur le pont local ; la phrase est normalisée par le modèle sur l'appareil du Mac quand il est disponible, sinon analysée telle quelle. Champ assistant dans la PWA.
- Variante moderne : couleurs conservées dans les sélecteurs, édition en ligne du journal qui ne valide qu'à la fin de la saisie, colonne « État » cliquable, calendrier en popover pour les dates, recherche repliée en loupe (⌘F), filtre de comptes seulement sur les pages qu'il filtre, filtre de comptes et soldes pointé / actuel dans la barre d'outils, sélecteurs en popover avec l'icône et la couleur de chaque entrée (catégories, types, états, budgets, fréquences, comptes), montants avec les centimes partout, menus de filtres illustrés, bulles de survol colorées, point bas journalier affiché par défaut, pied de barre latérale ancré en bas (Catégories, Paramètres, mise à jour, Quitter en rouge).
- Variante moderne : filtre de comptes visible en permanence (un bouton par compte, soldes pointé et actuel à droite), page Paramètres dans la fenêtre (⌘,), pied de barre latérale avec mise à jour et Quitter, filtres alignés à droite, cartes de la vue d'ensemble de taille égale, survol des graphiques avec bulle de valeurs, tables avec sélection et actions pointer / modifier / supprimer.
- Tous les formulaires de la variante moderne sont natifs, assistant d'import CSV / QIF / OFX, restauration `.dmx`, suggestions et nouveautés comprises.
- Deux builds macOS séparés comme en 1.x : « modern » pour Apple Silicon, avec une interface SwiftUI native macOS 26 (barre latérale et barre d'outils système, tables natives, Swift Charts, SF Symbols, fenêtre Réglages, Liquid Glass), et « legacy » pour les Mac Intel sous Catalina, avec l'interface compatible.

### Données et synchronisation

- Reprise automatique des données de DmxMoney 1.x au premier lancement (copie de la base et des certificats du pont, l'originale n'est jamais modifiée).
- Si le dossier de données contient déjà une base moins complète qu'une base 1.x voisine (dossier laissé par un ancien build), l'app le détecte et propose la reprise, après avoir exporté l'existante en `.dmx`.
- Le client PWA est embarqué dans les applications de bureau : le pont local le sert lui-même et le QR d'appairage pointe vers lui, au lieu de la PWA publique.
- Schéma SQLite et format `.dmx` compatibles 1.x : restauration complète ou fusion.
- Synchronisation au choix : iCloud (`CKSyncEngine`) entre appareils Apple, et/ou compagnon mobile PWA via le pont HTTPS existant, avec passkeys et appairage par QR. Les routes de l'API du pont sont inchangées : la PWA fonctionne sans modification.

### Corrections reprises de la 1.x

- Ajout de mois plafonné à la fin du mois (31 janvier + 1 mois donnait le 3 mars).
- Comparaison du mois **et** de l'année dans la vue d'ensemble.
- Les montants saisis avec des séparateurs de milliers (« 1 200,50 ») sont interprétés correctement.

### Distribution

- macOS : deux DMG signés et notarisés (Apple Silicon et Intel/Catalina), recherche de mise à jour intégrée par architecture.
- Windows : installeur et mises à jour Velopack (x64 et arm64).
- Linux : AppImage et Flatpak, icônes Lucide converties en icônes symboliques GTK.

## Versions 1.x

L'historique complet est conservé dans [docs/CHANGELOG-v1.md](docs/CHANGELOG-v1.md).
