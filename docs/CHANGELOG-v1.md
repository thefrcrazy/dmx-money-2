# Changelog

## 1.0.22 - 2026-09-01

- Correction majeure du trousseau : `keyring` était compilé sans backend natif et retombait sur un stockage en mémoire, donc le secret du pont était perdu à chaque redémarrage de l'application.
- Conséquence corrigée en chaîne : l'application ne tente plus de se reprovisionner en boucle, le certificat HTTPS se renouvelle à nouveau et l'appairage mobile refonctionne.
- Le statut du pont signale désormais un certificat expiré au lieu de l'afficher comme prêt.

## 1.0.21 - 2026-09-01

- Le QR d'appairage se génère à nouveau quand le service managé refuse le provisionnement : un pont déjà certifié n'est plus bloqué par un 401.
- Un appareil déjà enregistré peut renouveler ses accès avec son propre secret, sans dépendre du secret de provisionnement partagé.
- Un identifiant d'appareil existant n'est plus remplacé : le sous-domaine local et le certificat en place sont conservés au lieu d'être orphelins.
- Renouvellement automatique du DNS et du certificat toutes les 6 heures, pour survivre à un changement d'IP locale ou à plusieurs semaines sans ouvrir l'application.
- Le serveur local redémarre tout seul après un renouvellement de certificat au lieu de servir l'ancienne chaîne.
- La PWA retrouve le desktop quand celui-ci revient sur un autre port, et rejoue les modifications faites hors ligne au lieu de les abandonner.
- Deux mobiles peuvent rester appairés en même temps, même s'ils partagent un trousseau de clés d'accès ; leurs noms ne s'écrasent plus entre eux.
- Prédictions : les retraits sont calculés avant les revenus d'une même journée, avec un indicateur de point bas pour éviter les frais de découvert même quand la journée se termine en positif.

## 1.0.20 - 2026-07-04

- Suppression du faux bandeau d'erreur 401 quand le pont HTTPS est déjà actif et utilisable.
- Réutilisation du DNS et du certificat existants si le refresh managé est refusé mais que l'infrastructure locale est saine.
- Nettoyage automatique de l'ancienne erreur de provisioning lors de l'activation d'un pont déjà configuré.

## 1.0.19 - 2026-07-04

- Correction de l'activation du compagnon mobile quand le pont HTTPS possède déjà un DNS et un certificat valides.
- L'application n'essaie plus de reprovisionner le pont si le trousseau macOS refuse temporairement le secret device mais que le certificat local est prêt.
- Le statut du pont reconnaît maintenant une configuration existante récupérable sans afficher un faux état de provisioning.

## 1.0.18 - 2026-07-03

- Correction d'un blocage d'activation du compagnon mobile lorsque le pont HTTPS possédait déjà un appareil local valide.
- Réparation automatique des champs de configuration du pont depuis le device existant au lieu de reprovisionner inutilement.
- Conservation du modèle Worker fermé : le provisioning Cloudflare reste protégé par secret serveur.
- Rotation du secret de provisioning local/Cloudflare pour réaligner l'installation desktop actuelle.

## 1.0.17 - 2026-07-03

- Correction de la récupération du pont mobile HTTPS après plusieurs jours sans synchronisation.
- Conservation du même appareil et du même sous-domaine local lors d'un reprovisionnement sécurisé.
- Recréation automatique du record DNS Cloudflare si l'ancien identifiant est périmé ou supprimé.
- Restauration du déploiement Worker avec la zone Cloudflare correcte pour les mises à jour DNS.
- Renouvellement du cache PWA afin de distribuer immédiatement le correctif mobile.
- Correction des warnings Recharts lorsque des graphiques sont rendus dans un conteneur responsive sans largeur stable.

## 1.0.16 - 2026-06-20

- Les confirmations de suppression utilisent maintenant la même feuille mobile que les formulaires d'ajout.
- Les dialogues mobiles restent correctement ancrés en bas et au-dessus de la barre de navigation.
- Le glisser-déposer tactile démarre après un court maintien afin de préserver le défilement normal.
- Suppression d'une ancienne copie locale inutilisée du module Rust d'arrière-plan.
- Renouvellement du cache PWA pour distribuer immédiatement les corrections.

## 1.0.15 - 2026-06-19

- Remplacement des sauvegardes complètes des paramètres par des mutations partielles ciblées.
- Détection des conflits par champ afin qu'un appareil hors ligne ne remplace pas une valeur modifiée ailleurs.
- Fusion des transactions fictives par identifiant et protection contre les suppressions obsolètes.
- Conservation additive des suggestions ignorées et des catégories masquées entre desktop et PWA.
- Ordonnancement et regroupement de la file hors ligne avant la resynchronisation locale.

## 1.0.14 - 2026-06-18

- Correction de la réapparition répétée des nouveautés lors du passage entre la PWA mobile et l'application desktop.
- La version déjà consultée ne peut plus régresser lorsqu'un appareil synchronise des paramètres plus anciens.
- Fusion sécurisée de l'état de lecture entre le cache mobile, l'API compagnon et la base desktop.
- Renouvellement du cache PWA pour distribuer immédiatement le correctif aux applications installées.

## 1.0.13 - 2026-06-17

- Fermeture de la fenêtre principale sans arrêter l'application afin de garder le serveur compagnon actif en arrière-plan.
- Ajout d'un menu tray natif avec ouverture au clic gauche, raccourcis de navigation, accès mobile, paramètres et action Quitter.
- Affichage rapide des comptes dans le tray avec une ligne Tous et les soldes actuels par compte.
- Ajout d'un bouton Quitter dans la navigation desktop, masqué sur la PWA et branché sur une commande Rust dédiée.

## 1.0.12 - 2026-06-17

- Synchronisation des transactions fictives de Prédictions entre desktop et PWA mobile.
- Synchronisation du seuil d'alerte et des préférences de période de Prédictions.
- Synchronisation des préférences d'Analyses : période, dates personnalisées, démarrage au 1er du mois et catégories masquées.
- Synchronisation de la plage d'affichage de l'Échéancier.
- Migration automatique des anciennes préférences stockées localement vers les paramètres synchronisés.

## 1.0.11 - 2026-06-06

- Correction des selectors dans les formulaires : les listes ne sont plus coupées par les popups.
- Positionnement flottant partagé pour les selects simples et multi-selects, avec prise en compte du viewport mobile et des limites de formulaire.
- Ajout d'un verrou anti-clipping pendant l'ouverture des selectors pour éviter les conflits avec les animations et `overflow` des modales.

## 1.0.10 - 2026-06-02

- Refonte visuelle du changelog desktop et mobile avec une présentation plus compacte et plus minimaliste.
- Amélioration du contraste mobile pour éviter l'effet gris sur gris dans la fenêtre des nouveautés.
- Affichage du changelog via portal afin de rester au-dessus de la navigation mobile et des conteneurs de page.
- Conservation du contenu des notes de version avec une hiérarchie visuelle plus explicite.

## 1.0.9 - 2026-06-02

- Réglage du pull-to-refresh mobile pour éviter les rechargements pendant le scroll des tableaux, popups, champs et zones internes.
- Synchronisation des suggestions de budget et d'échéancier supprimées entre l'application desktop et la PWA mobile.
- Migration automatique des anciennes suggestions masquées depuis le stockage local vers les paramètres synchronisés.
- Ajout des champs de préférences synchronisées côté SQLite, API Tauri et API compagnon mobile.
- Rafraîchissement automatique des paramètres après une synchronisation mobile afin de garder desktop et PWA alignés.

## 1.0.8 - 2026-06-01

- Refonte de l'expérience mobile avec une interface plus proche d'une app native.
- Amélioration des modales mobiles : affichage plein écran correct, fond couvrant toute la page, scroll interne et animations d'ouverture/fermeture.
- Correction de la duplication visuelle du bouton de connexion PWA pendant la reprise de synchronisation.
- Correction de la génération idempotente des transactions d'échéances pour limiter les doublons après synchronisation.
- Refactor du backend Rust du mode compagnon et du pont sécurisé en modules dédiés pour faciliter la maintenance.

## 1.0.7 - 2026-05-29

- Ajout du mode compagnon mobile avec PWA installable, accès local HTTPS et QR d’appairage.
- Ajout d’un mode offline mobile avec stockage IndexedDB, queue de mutations et resynchronisation au retour du Wi-Fi local.
- Ajout de l’authentification mobile par clé d’accès/passkey, sessions courtes en cookie sécurisé et protection CSRF.
- Ajout du pont HTTPS managé via Cloudflare Worker, DNS local `*.sync.develop-max.com` et certificats ACME.
- Durcissement sécurité du bridge : provisioning protégé par secret serveur, suppression du token mobile legacy et nettoyage des anciens secrets SQLite.
- Amélioration de la PWA mobile : tutoriel d’installation, liaison par scan QR, déconnexion complète, reconnexion automatique par clé d’accès et priorité donnée à la passkey quand elle existe.
- Amélioration responsive mobile : navigation mobile, pull-to-refresh, tableaux plus robustes et masquage des fonctions desktop-only.

## 1.0.6 - 2026-05-21

- Ajout de transactions fictives dans Prédictions pour simuler des dépenses, revenus ou virements sans modifier le journal.
- Édition et suppression des simulations directement depuis la page Prédictions.
- Activation/désactivation individuelle des simulations via checkbox pour tester rapidement plusieurs scénarios.
- Conservation locale des simulations et compatibilité avec les anciennes simulations déjà enregistrées.

## 1.0.5 - 2026-05-18

- Correction du chargement SystemJS legacy sur macOS Catalina.
- Forçage de l'URL du bundle legacy vers `tauri://localhost/assets/...` dans le vieux WebKit.
- Conservation de Vite 8/Rolldown pour les builds modern et Apple Silicon.
- Build Mac Intel toujours réservé au preset legacy.

## 1.0.4 - 2026-05-18

- Correction du blocage CSP sur Mac Intel/Catalina.
- Autorisation explicite des assets `tauri://assets/...` dans la politique de sécurité.
- Conservation de l'inline CSS/JS nécessaire au bootstrap legacy SystemJS.
- Désactivation ciblée de la réécriture CSP Tauri sur `script-src` et `style-src`.

## 1.0.3 - 2026-05-18

- Correction du démarrage Mac Intel/Catalina avec assets Tauri/Vite en chemins relatifs.
- Séparation plus nette du build modern et du build legacy Intel.
- Suppression du bootstrap legacy inutile dans le build modern.
- Parsers CSV, QIF et OFX centralisés et plus tolérants.
- Ignorance des doublons évidents lors des imports bancaires.
- Suppression cohérente des deux lignes d'un virement lié.
- Activation de garde-fous SQLite et CSP Tauri plus stricte.
- Correction du libellé backup : le `.dmx` est un export local encodé, pas un chiffrement.

## 1.0.2 - 2026-05-18

- Remplacement du flux de mise à jour Windows par un setup NSIS signé directement.
- Suppression du chemin de mise à jour Windows basé sur un `.msi.zip` à décompresser.
- Conservation du build modern pour macOS Apple Silicon.
- Build legacy-only réservé aux Mac Intel/Catalina.
- Ajout d'un garde-fou contre l'écran de chargement infini au démarrage.
- Centralisation de l'état de l'updater pour éviter les vérifications concurrentes.
- Suppression des versions affichées en dur dans l'interface.
