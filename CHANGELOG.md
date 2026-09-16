# Changelog

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
