# Publier une version de DmxMoney 2

Deux workflows GitHub Actions :

* **`ci.yml`** — à chaque push et pull request : noyau Rust (format, clippy, tests), app Linux
  (clippy, build release, validation `.desktop`/AppStream), app Apple (bindings, tests DmxKit,
  builds macOS modern et legacy, iOS simulateur), app Windows (bindings, build, tests xUnit,
  ouverture de chaque page de l'app publiée). Les étapes de vérification échouent si les
  bindings Swift/C# ou les icônes natives (`scripts/gen-native-icons.py --check`) n'ont pas été
  régénérés après une modification du noyau ou de `shared/icons/native.json`.
* **`release.yml`** — sur un tag `v2.*` (ou déclenchement manuel) : deux DMG macOS signés et
  notarisés (`-apple-silicon` et `-intel-catalina`) + flux `updates.json`, installeurs Velopack
  x64 et arm64, AppImage, bundle Flatpak, puis création de la release GitHub avec les notes
  tirées du `CHANGELOG.md`.

## Vérification des interfaces à chaque version

Toute modification d’une interface doit être comparée aux implémentations macOS moderne,
macOS Intel/Catalina, Windows, Linux et PWA. Documenter les différences voulues et reporter
les corrections communes, sans supposer que la compilation d’une variante valide les autres.

- Vérifier les actions accessibles (onglets, mise à jour, import/export), les soldes et les formats.
- Contrôler les icônes en clair, sombre et après retour au clair, y compris une sélection active.
- La CI lance `scripts/verify-apple-ui.sh` : application AppKit, données fictives isolées,
  icônes embarquées forcées, navigation effective dans les quatre onglets des paramètres.
  Les captures sont conservées dans l’artefact `apple-ui-catalina-artwork` pour revue visuelle.
- Ce test sur un OS récent ne remplace pas un essai sur Catalina réel. Signaler explicitement
  lorsque l’ancien OS n’est pas disponible ; ne pas qualifier cette limite de validation Catalina.
- Conserver la cible Intel macOS 10.15. Le SDK local Xcode 27 peut nécessiter une cible de test
  12.0 (`DMXMONEY_UI_MIN_MACOS=12.0`), sans changer la cible des binaires distribués.

## Étapes

```bash
# 1. Mettre à jour la version partout (fichier VERSION = source de vérité)
echo 2.0.1 > VERSION
#    apple/project.yml (MARKETING_VERSION), linux/dmx-money-gtk/Cargo.toml, windows/…/Version
# 2. Compléter le CHANGELOG (section « ## 2.0.1 »)
# 3. Vérifier localement
cargo fmt --all --check && cargo clippy --workspace --all-targets && cargo test --workspace
# 4. Taguer
git tag v2.0.1 && git push origin v2.0.1
```

## Secrets attendus

Aucun secret n'est nécessaire pour la CI. Pour une release complète, tout est optionnel : les
étapes concernées sont simplement sautées si le secret est absent (build non signé, pas
d'appcast).

| Secret | Usage |
|---|---|
| `DMXMONEY_MANAGED_BRIDGE_REGISTRATION_SECRET` | secret d'enregistrement du pont managé, compilé dans le noyau (`option_env!`) |
| `DMXMONEY_MANAGED_BRIDGE_URL` | URL du Worker Cloudflare du pont managé |
| `APPLE_CERTIFICATE_P12` / `APPLE_CERTIFICATE_PASSWORD` | certificat Developer ID (base64) et son mot de passe |
| `APPLE_TEAM_ID` | équipe de signature ; active aussi les entitlements iCloud |
| `DMX_ICLOUD_CONTAINER` | conteneur CloudKit (`iCloud.com.dmxmoney.app`) |
| `APPLE_API_KEY` / `APPLE_API_KEY_ID` / `APPLE_API_ISSUER` | clé App Store Connect (base64) pour la notarisation |
| `DMX_UPDATE_FEED_URL` | URL du flux lu par l'app macOS, par exemple `https://github.com/<owner>/<repo>/releases/latest/download/updates.json` |

Sans ce secret, les builds utilisent le flux des releases GitHub du dépôt. Les deux variantes
macOS partagent `FileActions.settingsActions` : le bouton « Vérifier les mises à jour… »
reste accessible dans les paramètres et dans le menu de l’application.

## Mises à jour macOS

`scripts/update-feed.py` produit `updates.json`, publié comme asset de la release :

```json
{
  "version": "2.0.1",
  "notes": "- …",
  "platforms": {
    "darwin-arm64":  { "url": "https://…/DmxMoney-2.0.1-apple-silicon.dmg",  "minimumSystemVersion": "26.0" },
    "darwin-x86_64": { "url": "https://…/DmxMoney-2.0.1-intel-catalina.dmg", "minimumSystemVersion": "10.15" }
  }
}
```

L'app compare cette version à la sienne (au plus une vérification par jour, plus l'entrée de
menu), affiche les nouveautés puis permet de télécharger et remplacer l’application avec
copie de secours. L’installation manuelle du DMG reste disponible. Sparkle 2 n'est pas utilisé : ses binaires officiels exigent macOS 11, ce qui
empêcherait le lancement sous Catalina.

## Windows

`scripts/build-windows.ps1` produit la publication autonome puis l'installeur Velopack. Les
mises à jour se lisent depuis l'URL Velopack par défaut, remplaçable à l'exécution par
`DMXMONEY_UPDATE_URL`.

## Linux

* **AppImage** : `scripts/build-linux-appimage.sh` (icônes, build release, AppDir, appimagetool).
* **Flatpak** : `linux/flatpak/com.dmxmoney.app.yml`. Le manifeste télécharge les dépendances
  Cargo pendant le build (`--share=network`) ; pour Flathub, générer `cargo-sources.json` avec
  `flatpak-builder-tools/cargo` et retirer ce réglage.

Le client PWA (`pwa/dist`) est construit avant l'installation pour être servi par le pont local ;
sans lui, le pont expose seulement l'API.
