# DmxMoney — Windows (WinUI 3)

Application WinUI 3 non empaquetée, installée et mise à jour par [Velopack](https://velopack.io).
Toute la logique métier vit dans le noyau Rust partagé (`core/`) : l'interface n'affiche que ce
que le noyau calcule.

## Projets

| Projet | Rôle |
|---|---|
| `src/DmxMoney.Interop` | Bindings C# générés par `uniffi-bindgen-cs` + `dmx_ffi.dll` par architecture |
| `src/DmxMoney.ViewModels` | Modèles de vue (net10.0, CommunityToolkit.Mvvm) — **testables sur macOS et Linux** |
| `src/DmxMoney.App` | Interface WinUI 3 : NavigationView, tableaux, graphiques XAML, icône de barre des tâches |
| `tests/DmxMoney.Tests` | Tests xUnit du noyau et des modèles de vue |

Les graphiques sont dessinés avec les primitives XAML (`Path`, `Polyline`, `ArcSegment`) plutôt
qu'avec une bibliothèque tierce : même rendu que sur macOS et Linux, aucune dépendance de plus.

Les icônes sont celles de Windows : `AppIcon` affiche le glyphe Segoe Fluent Icons (Segoe MDL2
Assets sous Windows 10) du nom d'icône stocké en base. La correspondance vient de
`shared/icons/native.json`, générée dans `Icons/FluentIcons.g.cs` par `scripts/gen-native-icons.py`.

## Développer

```powershell
# Noyau Rust + bindings + publication + installeur
pwsh scripts/build-windows.ps1 -Rid win-x64

# Ou, pour itérer dans Visual Studio / dotnet
cargo build -p dmx-ffi --release --target x86_64-pc-windows-msvc
copy target\x86_64-pc-windows-msvc\release\dmx_ffi.dll windows\src\DmxMoney.Interop\runtimes\win-x64\native\
dotnet build windows\DmxMoney.slnx
```

Les tests des modèles de vue tournent sur n'importe quel système, avec la bibliothèque native locale :

```bash
./scripts/gen-csharp-bindings.sh   # construit dmx_ffi pour l'hôte et régénère les bindings
dotnet test windows/tests/DmxMoney.Tests/DmxMoney.Tests.csproj
```

## Données

* Base : `%APPDATA%\com.dmxmoney.app\dmxmoney2025.db`
* Reprise automatique de DmxMoney 1.x (`%APPDATA%\com.dmxmoney.desktop`) au premier lancement ;
  la base d'origine n'est jamais modifiée.
* `DMXMONEY_DATA_DIR` permet de travailler sur un dossier de test (pont PWA désactivé).
* `DMXMONEY_UPDATE_URL` remplace la source de mise à jour Velopack.
