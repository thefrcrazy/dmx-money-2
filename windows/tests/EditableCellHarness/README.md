Ce harnais exécute le véritable `EditableCell.cs` avec des stubs minimaux d’événements WinUI. Il couvre le changement d’identifiant, l’unbind, Échap, le changement de champ, le double commit et la conservation de la valeur initiale pour les conflits de synchronisation.

Il compile aussi le véritable `FormDialog.cs` et vérifie l’attente du résultat, la soumission unique, l’erreur visible dans le formulaire conservé, la désactivation des contrôles et le nettoyage des abonnements. Les stubs respectent la séparation WinUI : `ContentControl` possède `IsEnabled`, `ContentPresenter` n’en possède pas.

Il ne valide pas le focus, le recyclage visuel ou l’accessibilité dans un véritable runtime WinUI. Ces contrôles restent à vérifier sur Windows.

```sh
dotnet run --project windows/tests/EditableCellHarness/EditableCellHarness.csproj -p:UseSharedCompilation=false
```
