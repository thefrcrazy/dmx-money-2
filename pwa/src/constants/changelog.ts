export interface VersionUpdate {
    version: string;
    date: string;
    title: string;
    changes: string[];
    features?: {
        title: string;
        description: string;
        icon: string;
    }[];
}

export const CHANGELOG: VersionUpdate[] = [
    {
        version: "1.0.22",
        date: "2026-09-01",
        title: "Trousseau réparé, pont mobile rétabli",
        changes: [
            "Le secret du pont est de nouveau conservé dans le trousseau système entre deux lancements.",
            "Plus de tentatives de reprovisionnement en boucle causées par un secret introuvable.",
            "Le certificat HTTPS se renouvelle à nouveau automatiquement.",
            "Un certificat expiré est signalé au lieu d'être affiché comme prêt."
        ],
        features: [
            {
                title: "Secret enfin persistant",
                description: "Le trousseau système est réellement utilisé, au lieu d'un stockage en mémoire perdu à chaque redémarrage.",
                icon: "Key"
            },
            {
                title: "Certificat renouvelé",
                description: "Le renouvellement automatique refonctionne, donc l'appairage mobile aussi.",
                icon: "Shield"
            }
        ]
    },
    {
        version: "1.0.21",
        date: "2026-09-01",
        title: "Pont mobile increvable et prédictions plus sûres",
        changes: [
            "Le QR d'appairage se génère à nouveau quand le service managé refuse le provisionnement.",
            "Un appareil déjà enregistré renouvelle ses accès avec son propre secret, sans secret partagé.",
            "Le sous-domaine local et le certificat existants sont conservés au lieu d'être remplacés.",
            "Renouvellement automatique du DNS et du certificat toutes les 6 heures.",
            "La PWA retrouve le desktop revenu sur un autre port et rejoue les modifications hors ligne.",
            "Deux mobiles peuvent rester appairés en même temps sans écraser leurs noms.",
            "Prédictions : retraits calculés avant les revenus, avec indicateur de point bas journalier."
        ],
        features: [
            {
                title: "Appairage toujours possible",
                description: "Un pont déjà certifié reste appairable même si le service managé est injoignable.",
                icon: "Key"
            },
            {
                title: "Reconnexion automatique",
                description: "La PWA retrouve le desktop et ne perd plus les modifications faites hors ligne.",
                icon: "Wifi"
            },
            {
                title: "Deux mobiles appairés",
                description: "Téléphone et tablette peuvent rester liés en même temps au même desktop.",
                icon: "Smartphone"
            },
            {
                title: "Point bas journalier",
                description: "Un retrait avant un revenu le même jour est signalé, même si la journée finit en positif.",
                icon: "Shield"
            }
        ]
    },
    {
        version: "1.0.20",
        date: "2026-07-04",
        title: "Pont mobile sans fausse alerte",
        changes: [
            "Suppression du faux bandeau d'erreur 401 quand le pont HTTPS est déjà actif et utilisable.",
            "Réutilisation du DNS et du certificat existants si le refresh managé est refusé mais que l'infrastructure locale est saine.",
            "Nettoyage automatique de l'ancienne erreur de provisioning lors de l'activation d'un pont déjà configuré."
        ],
        features: [
            {
                title: "Erreur obsolète masquée",
                description: "Le statut ne remonte plus un ancien 401 quand l'API HTTPS locale fonctionne.",
                icon: "CheckCircle2"
            },
            {
                title: "Refresh plus tolérant",
                description: "Un pont déjà certifié reste utilisable même si le service managé refuse un reprovisionnement inutile.",
                icon: "ShieldCheck"
            }
        ]
    },
    {
        version: "1.0.19",
        date: "2026-07-04",
        title: "Pont mobile sans reprovisionnement",
        changes: [
            "Correction de l'activation du compagnon mobile quand le pont HTTPS possède déjà un DNS et un certificat valides.",
            "L'application n'essaie plus de reprovisionner le pont si le trousseau macOS refuse temporairement le secret device mais que le certificat local est prêt.",
            "Le statut du pont reconnaît maintenant une configuration existante récupérable sans afficher un faux état de provisioning."
        ],
        features: [
            {
                title: "Activation directe",
                description: "Un pont déjà certifié peut redémarrer sans repasser par Cloudflare.",
                icon: "ShieldCheck"
            },
            {
                title: "Moins de faux blocages",
                description: "Un accès trousseau temporairement refusé ne force plus un reprovisionnement inutile.",
                icon: "RefreshCw"
            }
        ]
    },
    {
        version: "1.0.18",
        date: "2026-07-03",
        title: "Activation mobile réparée",
        changes: [
            "Correction d'un blocage d'activation du compagnon mobile lorsque le pont HTTPS possède déjà un appareil local valide.",
            "Réparation automatique des champs de configuration du pont depuis le device existant au lieu de reprovisionner inutilement.",
            "Conservation du modèle Worker fermé : le provisioning Cloudflare reste protégé par secret serveur.",
            "Rotation du secret de provisioning local/Cloudflare pour réaligner l'installation desktop actuelle."
        ],
        features: [
            {
                title: "Pont existant conservé",
                description: "L'app évite un nouveau provisioning quand le device local peut déjà être récupéré.",
                icon: "ShieldCheck"
            },
            {
                title: "Activation plus stable",
                description: "Les champs manquants du pont sont reconstruits sans casser l'appairage mobile.",
                icon: "RefreshCw"
            }
        ]
    },
    {
        version: "1.0.17",
        date: "2026-07-03",
        title: "Pont mobile récupérable",
        changes: [
            "Correction de la récupération du pont mobile HTTPS après plusieurs jours sans synchronisation.",
            "Conservation du même appareil et du même sous-domaine local lors d'un reprovisionnement sécurisé.",
            "Recréation automatique du record DNS Cloudflare si l'ancien identifiant est périmé ou supprimé.",
            "Restauration du déploiement Worker avec la zone Cloudflare correcte pour les mises à jour DNS.",
            "Renouvellement du cache PWA afin de distribuer immédiatement le correctif mobile.",
            "Correction des warnings Recharts lorsque des graphiques sont rendus dans un conteneur responsive sans largeur stable."
        ],
        features: [
            {
                title: "Reconnexion mobile",
                description: "Le pont sécurisé peut récupérer une installation existante sans casser l'appairage PWA.",
                icon: "RefreshCw"
            },
            {
                title: "DNS robuste",
                description: "Un record Cloudflare périmé est recréé automatiquement au lieu de bloquer la sync.",
                icon: "Shield"
            }
        ]
    },
    {
        version: "1.0.16",
        date: "2026-06-20",
        title: "Interactions mobiles plus naturelles",
        changes: [
            "Les confirmations de suppression utilisent maintenant la même feuille mobile que les formulaires d'ajout.",
            "Les dialogues mobiles restent correctement ancrés en bas et au-dessus de la barre de navigation.",
            "Le glisser-déposer tactile démarre après un court maintien afin de préserver le défilement normal.",
            "Suppression d'une ancienne copie locale inutilisée du module Rust d'arrière-plan.",
            "Renouvellement du cache PWA pour distribuer immédiatement les corrections."
        ],
        features: [
            {
                title: "Confirmations cohérentes",
                description: "Les actions sensibles reprennent le même comportement que les autres feuilles mobiles.",
                icon: "CheckCircle2"
            },
            {
                title: "Drag tactile maîtrisé",
                description: "Un maintien volontaire déclenche le déplacement sans gêner le scroll.",
                icon: "Smartphone"
            }
        ]
    },
    {
        version: "1.0.15",
        date: "2026-06-19",
        title: "Synchronisation sans écrasement",
        changes: [
            "Remplacement des sauvegardes complètes des paramètres par des mutations partielles ciblées.",
            "Détection des conflits par champ afin qu'un appareil hors ligne ne remplace pas une valeur modifiée ailleurs.",
            "Fusion des transactions fictives par identifiant et protection contre les suppressions obsolètes.",
            "Conservation additive des suggestions ignorées et des catégories masquées entre desktop et PWA.",
            "Ordonnancement et regroupement de la file hors ligne avant la resynchronisation locale."
        ],
        features: [
            {
                title: "Aucune suppression implicite",
                description: "Les anciennes données mobiles ne peuvent plus effacer les réglages plus récents.",
                icon: "Shield"
            },
            {
                title: "Conflits maîtrisés",
                description: "Une modification distante est conservée lorsque la valeur locale est devenue obsolète.",
                icon: "ArrowRightLeft"
            }
        ]
    },
    {
        version: "1.0.14",
        date: "2026-06-18",
        title: "Nouveautés synchronisées sans boucle",
        changes: [
            "Correction de la réapparition répétée des nouveautés lors du passage entre la PWA mobile et l'application desktop.",
            "La version déjà consultée ne peut plus régresser lorsqu'un appareil synchronise des paramètres plus anciens.",
            "Fusion sécurisée de l'état de lecture entre le cache mobile, l'API compagnon et la base desktop.",
            "Renouvellement du cache PWA pour distribuer immédiatement le correctif aux applications installées."
        ],
        features: [
            {
                title: "Lecture mémorisée",
                description: "Une nouveauté validée reste validée sur les appareils synchronisés.",
                icon: "CheckCircle2"
            },
            {
                title: "Sync protégée",
                description: "Un cache ancien ne peut plus réactiver une note de version déjà consultée.",
                icon: "RefreshCw"
            }
        ]
    },
    {
        version: "1.0.13",
        date: "2026-06-17",
        title: "Mode arrière-plan et tray natif",
        changes: [
            "Fermeture de la fenêtre principale sans arrêter l'application afin de garder le serveur compagnon actif en arrière-plan.",
            "Ajout d'un menu tray natif avec ouverture au clic gauche, raccourcis de navigation, accès mobile, paramètres et action Quitter.",
            "Affichage rapide des comptes dans le tray avec une ligne Tous et les soldes actuels par compte.",
            "Ajout d'un bouton Quitter dans la navigation desktop, masqué sur la PWA et branché sur une commande Rust dédiée."
        ],
        features: [
            {
                title: "App toujours active",
                description: "Le compagnon mobile peut rester disponible tant que DmxMoney tourne en arrière-plan.",
                icon: "Activity"
            },
            {
                title: "Tray plus utile",
                description: "Le menu système donne accès aux comptes, raccourcis et actions essentielles sans rouvrir toute l'interface.",
                icon: "Menu"
            }
        ]
    },
    {
        version: "1.0.12",
        date: "2026-06-17",
        title: "Préférences métier synchronisées",
        changes: [
            "Synchronisation des transactions fictives de Prédictions entre desktop et PWA mobile.",
            "Synchronisation du seuil d'alerte et des préférences de période de Prédictions.",
            "Synchronisation des préférences d'Analyses : période, dates personnalisées, démarrage au 1er du mois et catégories masquées.",
            "Synchronisation de la plage d'affichage de l'Échéancier.",
            "Migration automatique des anciennes préférences stockées localement vers les paramètres synchronisés."
        ],
        features: [
            {
                title: "Prédictions alignées",
                description: "Les simulations et seuils affichent le même résultat sur desktop et sur la PWA.",
                icon: "TrendingUp"
            },
            {
                title: "Préférences partagées",
                description: "Analyses et Échéancier conservent leurs réglages entre les appareils appairés.",
                icon: "RefreshCw"
            }
        ]
    },
    {
        version: "1.0.11",
        date: "2026-06-06",
        title: "Selectors non coupés",
        changes: [
            "Correction des selectors dans les formulaires : les listes ne sont plus coupées par les popups.",
            "Positionnement flottant partagé pour les selects simples et multi-selects, avec prise en compte du viewport mobile et des limites de formulaire.",
            "Ajout d'un verrou anti-clipping pendant l'ouverture des selectors pour éviter les conflits avec les animations et l'overflow des modales."
        ],
        features: [
            {
                title: "Formulaires plus fiables",
                description: "Les listes de sélection restent visibles dans les popups, même près du bas du formulaire.",
                icon: "CheckCircle2"
            },
            {
                title: "Positionnement intelligent",
                description: "Les selectors s'adaptent au viewport et s'ouvrent dans le sens qui garde les options accessibles.",
                icon: "ChevronDown"
            }
        ]
    },
    {
        version: "1.0.10",
        date: "2026-06-02",
        title: "Changelog plus lisible",
        changes: [
            "Refonte visuelle du changelog desktop et mobile avec une présentation plus compacte et plus minimaliste.",
            "Amélioration du contraste mobile pour éviter l'effet gris sur gris dans la fenêtre des nouveautés.",
            "Affichage du changelog via portal afin de rester au-dessus de la navigation mobile et des conteneurs de page.",
            "Conservation du contenu des notes de version avec une hiérarchie visuelle plus explicite."
        ],
        features: [
            {
                title: "Nouveautés plus claires",
                description: "Les notes de version sont plus faciles à parcourir, avec des repères visuels plus sobres.",
                icon: "Sparkles"
            },
            {
                title: "Mobile mieux contrasté",
                description: "Le panneau mobile utilise un fond plus lumineux et des cartes mieux séparées.",
                icon: "Smartphone"
            }
        ]
    },
    {
        version: "1.0.9",
        date: "2026-06-02",
        title: "Synchronisation mobile et gestes plus précis",
        changes: [
            "Réglage du pull-to-refresh mobile pour éviter les rechargements pendant le scroll des tableaux, popups, champs et zones internes.",
            "Synchronisation des suggestions de budget et d'échéancier supprimées entre l'application desktop et la PWA mobile.",
            "Migration automatique des anciennes suggestions masquées depuis le stockage local vers les paramètres synchronisés.",
            "Ajout des champs de préférences synchronisées côté SQLite, API Tauri et API compagnon mobile.",
            "Rafraîchissement automatique des paramètres après une synchronisation mobile afin de garder desktop et PWA alignés."
        ],
        features: [
            {
                title: "Suggestions synchronisées",
                description: "Une suggestion supprimée sur desktop ou mobile reste masquée sur l'autre appareil après synchronisation.",
                icon: "Sparkles"
            },
            {
                title: "Gestes mobiles fiabilisés",
                description: "Le rafraîchissement par glissement ne se déclenche plus lors du scroll d'une popup ou d'un tableau.",
                icon: "RefreshCw"
            }
        ]
    },
    {
        version: "1.0.8",
        date: "2026-06-01",
        title: "Expérience mobile et maintenance du compagnon",
        changes: [
            "Refonte de l'expérience mobile avec une interface plus proche d'une app native.",
            "Amélioration des modales mobiles : affichage plein écran correct, fond couvrant toute la page, scroll interne et animations d'ouverture/fermeture.",
            "Correction de la duplication visuelle du bouton de connexion PWA pendant la reprise de synchronisation.",
            "Correction de la génération idempotente des transactions d'échéances pour limiter les doublons après synchronisation.",
            "Refactor du backend Rust du mode compagnon et du pont sécurisé en modules dédiés pour faciliter la maintenance."
        ],
        features: [
            {
                title: "Popup mobile plus native",
                description: "Les formulaires mobiles s'ouvrent en bottom sheet, couvrent correctement l'écran et se ferment avec une animation fluide.",
                icon: "Smartphone"
            },
            {
                title: "Backend compagnon clarifié",
                description: "Le serveur compagnon et le pont sécurisé sont organisés en modules spécialisés pour préparer les prochaines évolutions.",
                icon: "Server"
            }
        ]
    },
    {
        version: "1.0.7",
        date: "2026-05-29",
        title: "PWA mobile et pont HTTPS sécurisé",
        changes: [
            "Ajout du mode compagnon mobile avec PWA installable, QR d’appairage et API locale HTTPS.",
            "Ajout du mode offline mobile avec IndexedDB, file de mutations locale et resynchronisation automatique au retour du Wi-Fi local.",
            "Ajout de l’authentification mobile par clé d’accès/passkey, session courte en cookie sécurisé et protection CSRF.",
            "Ajout du pont HTTPS managé via Cloudflare Worker avec DNS local `*.sync.develop-max.com` et certificats ACME.",
            "Durcissement du bridge : enregistrement d’appareil protégé par secret serveur, suppression du token mobile legacy et nettoyage des anciens secrets SQLite.",
            "Amélioration de l’expérience PWA : tutoriel d’installation iOS/Android, scan QR, déconnexion complète et reconnexion automatique avec la clé d’accès.",
            "Amélioration responsive mobile avec navigation adaptée, pull-to-refresh, tableaux plus robustes et masquage des fonctions desktop-only."
        ],
        features: [
            {
                title: "Compagnon mobile sécurisé",
                description: "La PWA peut fonctionner hors ligne puis se synchroniser automatiquement avec l’app desktop sur le réseau local sécurisé.",
                icon: "Smartphone"
            },
            {
                title: "Clé d’accès prioritaire",
                description: "Après appairage, la PWA privilégie la passkey et tente une reconnexion automatique lorsque la session expire.",
                icon: "KeyRound"
            }
        ]
    },
    {
        version: "1.0.6",
        date: "2026-05-21",
        title: "Prévisions avec simulations",
        changes: [
            "Ajout de transactions fictives dans Prédictions pour simuler des dépenses, revenus ou virements sans modifier le journal.",
            "Édition et suppression des simulations directement depuis la page Prédictions.",
            "Activation/désactivation individuelle des simulations via checkbox pour tester rapidement plusieurs scénarios.",
            "Conservation locale des simulations et compatibilité avec les anciennes simulations déjà enregistrées."
        ],
        features: [
            {
                title: "Scénarios de trésorerie",
                description: "Les prévisions peuvent intégrer des transactions fictives activables à la demande pour comparer plusieurs hypothèses.",
                icon: "TrendingUp"
            }
        ]
    },
    {
        version: "1.0.5",
        date: "2026-05-18",
        title: "Correctif Catalina SystemJS",
        changes: [
            "Correction de la résolution du bundle legacy SystemJS sur le protocole `tauri://` de macOS Catalina.",
            "Forçage de l'import legacy vers `tauri://localhost/assets/...` pour éviter le fallback HTML et l'erreur `Unexpected token '<'`.",
            "Conservation de Vite 8/Rolldown pour les builds modern et Apple Silicon.",
            "Maintien du build legacy-only pour les Mac Intel/Catalina."
        ],
        features: [
            {
                title: "Catalina plus compatible",
                description: "Le vieux WebKit charge l'entry legacy depuis l'origine Tauri correcte.",
                icon: "Monitor"
            }
        ]
    },
    {
        version: "1.0.4",
        date: "2026-05-18",
        title: "Correctif CSP Mac Intel",
        changes: [
            "Correction du blocage CSP qui empêchait le bundle legacy Mac Intel/Catalina de charger `tauri://assets/...`.",
            "Autorisation explicite du protocole Tauri dans les directives scripts, styles, images et polices.",
            "Conservation des styles et scripts inline nécessaires au bootstrap legacy SystemJS.",
            "Désactivation ciblée de la réécriture CSP Tauri pour `script-src` et `style-src`."
        ],
        features: [
            {
                title: "Legacy Intel débloqué",
                description: "Le vieux WebKit peut charger le bundle legacy sans rejet CSP.",
                icon: "Shield"
            }
        ]
    },
    {
        version: "1.0.3",
        date: "2026-05-18",
        title: "Correctifs Mac Intel & imports",
        changes: [
            "Correction du démarrage Mac Intel/Catalina avec des chemins d'assets relatifs dans les builds Tauri.",
            "Séparation plus nette du build modern et du build legacy : Apple Silicon conserve le build modern, Intel utilise un bundle legacy-only.",
            "Suppression du bootstrap legacy inutile dans le build modern pour éviter les erreurs de détection de modules sur WebKit ancien.",
            "Centralisation et fiabilisation des parsers CSV, QIF et OFX.",
            "Détection des doublons évidents lors des imports bancaires.",
            "Suppression cohérente des deux lignes d'un virement lié.",
            "Durcissement SQLite et CSP Tauri, avec correction du libellé backup."
        ],
        features: [
            {
                title: "Démarrage Intel fiabilisé",
                description: "Les anciens Mac Intel chargent le bundle legacy avec des chemins relatifs adaptés au packaging Tauri.",
                icon: "Monitor"
            },
            {
                title: "Imports plus robustes",
                description: "Les fichiers CSV, QIF et OFX sont mieux parsés et les doublons simples sont ignorés.",
                icon: "Upload"
            }
        ]
    },
    {
        version: "1.0.2",
        date: "2026-05-18",
        title: "Correctif updater Windows & Catalina",
        changes: [
            "Remplacement du flux de mise à jour Windows par un installateur NSIS signé directement, sans archive MSI zip à extraire.",
            "Séparation plus stricte des builds macOS : build modern conservé pour Apple Silicon et build legacy-only pour Intel/Catalina.",
            "Correction du risque d'écran de chargement infini lors du démarrage si les paramètres locaux ne répondent pas.",
            "Centralisation de l'état de mise à jour pour éviter les vérifications concurrentes dans l'interface.",
            "Nettoyage des versions affichées en dur : l'interface lit maintenant la version exposée par Tauri."
        ],
        features: [
            {
                title: "Mises à jour Windows fiabilisées",
                description: "Le nouvel artefact de mise à jour évite le chemin fragile de décompression du fichier MSI zip.",
                icon: "RefreshCw"
            },
            {
                title: "Compatibilité Mac préservée",
                description: "Les Mac Apple Silicon gardent le build modern, tandis que les anciens Mac Intel utilisent le build legacy.",
                icon: "Monitor"
            }
        ]
    },
    {
        version: "1.0.1",
        date: "2026-05-15",
        title: "UX/UI et Refonte des Paramètres",
        changes: [
            "Refonte complète de la page Paramètres avec un design natif iOS/macOS (Inset Grouped Lists).",
            "Mise à jour des couleurs et contrôles pour la sélection des thèmes et des couleurs d'accentuation.",
            "Ajout de micro-animations fluides dans l'interface de paramétrage."
        ],
        features: [
            {
                title: "Paramètres Premium",
                description: "Une toute nouvelle page de paramètres, aérée, épurée et très réactive, pensée comme une interface macOS native.",
                icon: "Settings"
            }
        ]
    },
    {
        version: "1.0.0",
        date: "2026-05-15",
        title: "Version 1.0 — Budget, Échéancier et Prévisions",
        changes: [
            "Refonte complète de la page Budget avec budgets indépendants, suivi par catégorie, détails minimalistes et intégration des échéances liées.",
            "Ajout de suggestions de budget et d'échéancier basées sur les opérations récurrentes du Journal, avec ajout et suppression des suggestions.",
            "Amélioration du Journal : tri stable des transactions à date identique, recherche sur toutes les colonnes, filtres multi-catégories et affichage du budget restant.",
            "Évolution de l'Échéancier : liaison explicite à un budget configuré, filtres améliorés et meilleure cohérence des formulaires.",
            "Ajout du scroll virtuel sur les tableaux pour améliorer les performances avec de gros volumes de données.",
            "Amélioration des pages Analyses et Prédictions avec filtre 2 mois, mémorisation des périodes, option de démarrage au 1er du mois et affichage journalier cohérent.",
            "Ajout d'un seuil d'alerte configurable dans Prédictions, avec alertes orange pour le seuil personnalisé et rouge pour les soldes négatifs.",
            "Uniformisation des boutons, des filtres et des états visuels sur les pages principales.",
            "Amélioration de l'intégration macOS : position des traffic lights, sidebar réduite, chevron, tooltips et titres de groupes.",
            "Mises à jour backend et base de données pour supporter les budgets autonomes, les liaisons budget-échéance et les nouveaux états de l'application."
        ],
        features: [
            {
                title: "Budget exploitable",
                description: "Les budgets peuvent maintenant vivre seuls ou être liés à des échéances, avec un suivi clair du prévu, dépensé et restant.",
                icon: "Target"
            },
            {
                title: "Prévisions plus lisibles",
                description: "Les projections affichent les jours, les seuils configurables et les alertes visuelles sans confondre seuil personnalisé et solde négatif.",
                icon: "TrendingUp"
            },
            {
                title: "Interface stabilisée",
                description: "Les tableaux, filtres, boutons et la sidebar macOS ont été harmonisés pour une utilisation plus fluide au quotidien.",
                icon: "Settings"
            }
        ]
    },
    {
        version: "0.7.3",
        date: "2026-03-11",
        title: "Correctif d'urgence (Données invisibles)",
        changes: [
            "Correction d'un problème critique où l'application pointait vers un mauvais dossier de données suite à la mise à jour 0.7.2.",
            "Vos données n'ont pas été supprimées ! Cette mise à jour rétablit simplement le lien vers votre base de données existante."
        ],
        features: [
            {
                title: "Restauration des données",
                description: "Correction de l'identifiant de l'application permettant de retrouver l'accès à vos comptes et transactions.",
                icon: "AlertCircle"
            }
        ]
    },
    {
        version: "0.7.2",
        date: "2026-03-11",
        title: "Transactions & Transferts",
        changes: [
            "Ajout du support complet des virements entre comptes depuis le Journal",
            "Nouveau sélecteur de type de transaction (Dépense, Revenu, Virement) dans le Journal",
            "Amélioration du formulaire d'ajout manuel avec sélection du compte source/destination",
            "Harmonisation de l'interface de saisie entre l'Échéancier et le Journal"
        ],
        features: [
            {
                title: "Virements facilités",
                description: "Gérez directement les virements entre vos différents comptes depuis la page Journal avec création automatique des deux transactions liées.",
                icon: "ArrowRightLeft"
            }
        ]
    },
    {
        version: "0.7.1",
        date: "2026-02-12",
        title: "Configuration & Identité",
        changes: [
            "Officialisation du nom de projet 'dmxmoney'",
            "Mise à jour des fichiers de configuration internes",
            "Nettoyage des références à l'ancien nom de code"
        ],
        features: []
    },
    {
        version: "0.7.0",
        date: "2026-02-11",
        title: "Saisie Intelligente & Calendrier",
        changes: [
            "Nouveau masque de saisie intelligent pour les dates (JJ/MM/AAAA) avec gestion du curseur",
            "Support complet de la saisie manuelle au clavier en plus du sélecteur",
            "Remplacement de Flatpickr par un calendrier natif optimisé pour la performance",
            "Restoration du design des bordures arrondies sur les modales",
            "Correction des couleurs de fond en mode compatibilité (Legacy)"
        ],
        features: []
    },
    {
        version: "0.5.6",
        date: "2026-02-11",
        title: "Correctif Windows & Styles",
        changes: [
            "Correction critique du système de mise à jour sur Windows (support 7z)",
            "Ajustements mineurs des styles globaux et des inputs",
            "Amélioration de la stabilité du calendrier sur les anciens navigateurs"
        ],
        features: []
    },
    {
        version: "0.5.5",
        date: "2026-02-11",
        title: "Identité visuelle & Nettoyage",
        changes: [
            "Mise à jour complète des icônes d'application pour toutes les plateformes (Windows, macOS, Android, iOS)",
            "Amélioration de la résolution du logo et nettoyage des ressources obsolètes",
            "Optimisation de la structure du projet et des types Vite",
            "Ajustements mineurs de l'interface utilisateur pour une meilleure cohérence"
        ],
        features: []
    },
    {
        version: "0.5.4",
        date: "2026-02-11",
        title: "Compatibilité Ultime & Calendrier",
        changes: [
            "Intégration de Flatpickr pour un calendrier fonctionnel sur tous les systèmes",
            "Correction du rendu des couleurs sur Safari 13 (Catalina) via syntaxe RGB legacy",
            "Saisies de dates désormais en format français (JJ/MM/AAAA)",
            "Sécurisation des fonctions de formatage pour les anciens moteurs JavaScript"
        ],
        features: []
    },
    {
        version: "0.5.3",
        date: "2026-02-11",
        title: "Correction Thème & Calendrier",
        changes: [
            "Correction du mode sombre et des couleurs personnalisées en mode Legacy",
            "Amélioration de la saisie des dates sur les anciens systèmes (macOS Catalina)",
            "Correction du centrage du logo sur l'écran de chargement",
            "Optimisation de la stabilité des variables CSS pour toutes les versions de Tailwind"
        ],
        features: []
    },
    {
        version: "0.5.2",
        date: "2026-02-11",
        title: "Compatibilité Intel & Tailwind Fix",
        changes: [
            "Correction majeure du build Intel (x64) pour macOS Catalina",
            "Force l'utilisation du mode Legacy pour les anciens systèmes Mac",
            "Synchronisation des configurations Vite pour éviter les écrasements de paramètres",
            "Mise à jour du pipeline de déploiement automatique"
        ],
        features: []
    },
    {
        version: "0.5.1",
        date: "2026-02-11",
        title: "Compatibilité Catalina & Optimisation",
        changes: [
            "Correction du crash au démarrage sur macOS Catalina (Intel)",
            "Renforcement de la transpilation Legacy pour les anciens moteurs WebKit",
            "Suppression des systèmes de cache de build instables",
            "Optimisation de la minification pour Safari"
        ],
        features: []
    },
    {
        version: "0.5.0",
        date: "2026-02-11",
        title: "Mise à jour majeure : Stabilité macOS",
        changes: [
            "Refonte du système de démarrage pour une stabilité maximale sur macOS",
            "Suppression définitive du flash blanc pour les utilisateurs en mode sombre",
            "Restauration intelligente de la taille et position de la fenêtre après le chargement",
            "Nettoyage complet de l'interface utilisateur pour un look plus épuré",
            "Optimisation des permissions système pour la gestion des fenêtres"
        ],
        features: []
    },
    {
        version: "0.4.7",
        date: "2026-02-11",
        title: "Splash Screen & Interface",
        changes: [
            "Splash screen repassé en 400x400 pour une transition plus stable",
            "Correction du bug d'interface écrasée après le démarrage",
            "Restauration garantie des bordures et contrôles de fenêtre",
            "Amélioration visuelle du logo et du spinner de chargement"
        ],
        features: []
    },
    {
        version: "0.4.5",
        date: "2026-02-11",
        title: "Perfectionnement du démarrage",
        changes: [
            "Correction de la taille du splash screen (120x120)",
            "Activation des ombres natives au démarrage",
            "Intégration du logo Or & Argent dans toute l'interface",
            "Stabilisation de la transition et des bordures de fenêtre"
        ],
        features: []
    },
    {
        version: "0.4.4",
        date: "2026-02-11",
        title: "Démarrage Premium",
        changes: [
            "Splash screen miniature (120x120) pour un chargement discret et élégant",
            "Transition animée prolongée (2s) pour une meilleure expérience utilisateur",
            "Uniformisation du logo dans toute l'application",
            "Gestion intelligente des boutons de fenêtre macOS au démarrage"
        ],
        features: []
    },
    {
        version: "0.4.3",
        date: "2026-02-11",
        title: "Finalisation des icônes",
        changes: [
            "Mise à jour complète de tous les formats d'icônes système (icns, ico, png)",
            "L'icône Gold & Silver est désormais visible dans le Dock et la barre des tâches"
        ],
        features: []
    },
    {
        version: "0.4.2",
        date: "2026-02-11",
        title: "Design Premium & Robustesse",
        changes: [
            "Nouveau logo Or et Argent pour une esthétique haut de gamme",
            "Correction définitive de la restauration des bordures de fenêtre",
            "Amélioration de la fluidité de transition après le splash screen"
        ],
        features: []
    },
    {
        version: "0.4.1",
        date: "2026-02-11",
        title: "Optimisation Turbo du Build",
        changes: [
            "Implémentation de sccache pour une compilation Rust ultra-rapide",
            "Amélioration des clés de cache pour éviter les recompilations inutiles",
            "Optimisation de la gestion des dépendances Bun"
        ],
        features: []
    },
    {
        version: "0.4.0",
        date: "2026-02-11",
        title: "Correctif de l'interface",
        changes: [
            "Correction du problème de barre de titre manquante après le splash screen",
            "Stabilisation de la transition entre le chargement et l'application",
            "Amélioration de la restauration de la position de la fenêtre"
        ],
        features: []
    },
    {
        version: "0.3.9",
        date: "2026-02-10",
        title: "Optimisation de l'infrastructure",
        changes: [
            "Accélération majeure du processus de build multi-plateforme",
            "Mise en cache intelligente des dépendances Rust et Frontend",
            "Réduction du temps d'attente pour les nouvelles releases"
        ],
        features: []
    },
    {
        version: "0.3.8",
        date: "2026-02-10",
        title: "Transition Dynamique",
        changes: [
            "Ajout d'une transition animée entre le splash screen et l'application",
            "Effet de zoom et de fondu fluide au démarrage",
            "Apparition progressive de l'interface principale"
        ],
        features: []
    },
    {
        version: "0.3.7",
        date: "2026-02-10",
        title: "Splash Screen Perfectionné",
        changes: [
            "Le splash screen est désormais un carré parfait sans bordures (borderless)",
            "Ajout de coins arrondis pour un aspect plus moderne au démarrage",
            "Transition améliorée vers l'interface principale"
        ],
        features: []
    },
    {
        version: "0.3.6",
        date: "2026-02-10",
        title: "Identité visuelle rafraîchie",
        changes: [
            "Nouveau logo professionnel au format SVG haute définition",
            "Mise à jour du splash screen avec le nouveau design",
            "Icône plus nette et moderne sur toute l'interface"
        ],
        features: []
    },
    {
        version: "0.3.5",
        date: "2026-02-10",
        title: "Splash Screen optimisé",
        changes: [
            "Le splash screen s'affiche désormais dans une fenêtre carrée centrée",
            "Transition fluide de la fenêtre splash vers la taille normale de l'application",
            "Amélioration visuelle du chargement initial"
        ],
        features: []
    },
    {
        version: "0.3.4",
        date: "2026-02-10",
        title: "Mises à jour intelligentes",
        changes: [
            "Système de mise à jour plus robuste : l'application ne propose plus de mise à jour tant qu'elle n'est pas 100% prête",
            "Amélioration des messages d'erreur lors des vérifications manuelles",
            "Mise à jour en arrière-plan plus discrète"
        ],
        features: []
    },
    {
        version: "0.3.3",
        date: "2026-02-10",
        title: "Nouvel écran de chargement",
        changes: [
            "Ajout d'un écran de chargement (splash screen) au démarrage",
            "Initialisation instantanée de la fenêtre",
            "Transition fluide entre le chargement et l'application"
        ],
        features: []
    },
    {
        version: "0.3.2",
        date: "2026-02-10",
        title: "Lancement fluide et instantané",
        changes: [
            "Suppression du 'flash' blanc/bleu au démarrage : l'application s'affiche désormais directement avec votre thème",
            "Optimisation du processus de chargement pour une meilleure réactivité"
        ],
        features: []
    },
    {
        version: "0.3.1",
        date: "2026-02-10",
        title: "Correctif de stabilité des paramètres",
        changes: [
            "Correction définitive de la perte des paramètres (thème, couleurs) au démarrage",
            "Correction de l'affichage répétitif des nouveautés à chaque lancement",
            "Amélioration de la synchronisation entre la fenêtre et la base de données"
        ],
        features: []
    },
    {
        version: "0.3.0",
        date: "2026-02-10",
        title: "Simplification de l'interface",
        changes: [
            "Regroupement des sections 'À propos' et 'Mises à jour' pour une navigation plus fluide",
            "Optimisation de l'espace dans les paramètres"
        ],
        features: []
    },
    {
        version: "0.2.9",
        date: "2026-02-10",
        title: "Nettoyage des paramètres",
        changes: [
            "Suppression du sélecteur de style manuel (désormais entièrement automatique au build)",
            "Optimisation de la structure des données de configuration"
        ],
        features: []
    },
    {
        version: "0.2.8",
        date: "2026-02-10",
        title: "Correction critique du crash des paramètres",
        changes: [
            "Correction d'un crash dans la page des paramètres (icône manquante)",
            "Optimisation du rendu des graphiques"
        ],
        features: []
    },
    {
        version: "0.2.7",
        date: "2026-02-10",
        title: "Correctif de persistence et d'affichage",
        changes: [
            "Correction de la perte des paramètres utilisateur lors des mises à jour",
            "Synchronisation complète des données entre l'interface et la base de données",
            "Ajout d'un sélecteur de style d'affichage (Moderne / Classique) dans les paramètres",
            "Amélioration de la stabilité de la sauvegarde des préférences"
        ],
        features: [
            {
                title: "Paramètres sauvegardés",
                description: "Vos préférences de thème, couleurs et organisation des comptes sont maintenant conservées durablement.",
                icon: "Settings"
            }
        ]
    },
    {
        version: "0.2.4",
        date: "2026-02-10",
        title: "Amélioration de l'expérience utilisateur",
        changes: [
            "Refonte visuelle des messages d'erreur",
            "Ajout d'explications pédagogiques lors des erreurs",
            "Possibilité de voir les détails techniques des erreurs",
            "Nouvelle liste de catégories par défaut plus complète",
            "Suppression des comptes de test par défaut"
        ],
        features: [
            {
                title: "Gestion des erreurs améliorée",
                description: "Les messages d'erreur sont plus clairs et vous aident à comprendre ce qui ne va pas.",
                icon: "AlertCircle"
            },
            {
                title: "Catégories enrichies",
                description: "Près de 30 catégories sont maintenant disponibles pour classer vos dépenses précisément.",
                icon: "Tag"
            }
        ]
    }
];

export const LATEST_VERSION = CHANGELOG[0].version;
