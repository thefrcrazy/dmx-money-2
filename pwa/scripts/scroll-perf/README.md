# Banc du journal

Ce banc charge le vrai composant Transactions, React en production et les styles de l'application, avec un contexte bancaire factice. Aucune connexion au relais ni modification de la banque réelle. Les écritures du banc restent en mémoire.

Depuis `pwa/` :

```sh
SCROLL_PERF_OUT_DIR=/private/tmp/dmx-scroll-perf-dist bunx --no-install vite build --config scripts/scroll-perf/vite.config.ts
python3 -m http.server 4178 --bind 127.0.0.1 --directory /private/tmp/dmx-scroll-perf-dist
```

Ouvrir `http://127.0.0.1:4178/?count=30000`, viewport mobile 390×844 ou desktop 1440×900. Le dossier de sortie par défaut, sans variable, est `dmx-scroll-perf-dist` dans le répertoire temporaire du système.

Vérifier le début, le milieu et la fin du journal, la recherche « Zèbre terminal unique », le pointage, la sélection, les résultats vides et Tab / Maj+Tab. L'éditeur desktop doit garder son brouillon lorsque sa ligne quitte la fenêtre visible.

Les fonctions de fixture `window.__fixturePrepend()` et `window.__fixtureDelete(id)` simulent une mise à jour distante en mémoire. La première opération visible doit garder sa position lors d'un ajout/suppression au-dessus de celle-ci.

Le nombre de lignes montées est `document.querySelectorAll('[data-transaction-id]').length` ; le nombre total de nœuds est `document.querySelectorAll('*').length`. Ces valeurs incluent les buffers autour du viewport ; un contrôle conservant le focus peut maintenir une ligne supplémentaire.

Ce banc n'est pas une route de l'application publiée. Il mesure le rendu, sans réseau ni SQL. Comparer les mêmes dimensions, le même nombre d'opérations et le même build production. Ne pas déduire des FPS d'un nombre de nœuds ou d'un benchmark de formatage.
