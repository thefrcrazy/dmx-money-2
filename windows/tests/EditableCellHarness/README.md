Ce harnais exécute le véritable `EditableCell.cs` avec des stubs minimaux d’événements WinUI. Il couvre le changement d’identifiant, l’unbind, Échap, le changement de champ, le double commit et la conservation de la valeur initiale pour les conflits de synchronisation.

Il ne valide pas le focus, le recyclage visuel ou l’accessibilité dans un véritable runtime WinUI. Ces contrôles restent à vérifier sur Windows.

```sh
dotnet run --project windows/tests/EditableCellHarness/EditableCellHarness.csproj -p:UseSharedCompilation=false
```
