using DmxMoney.Interop;
using DmxMoney.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace DmxMoney.App;

/// <summary>
/// Gestionnaires des gabarits de formulaires. Le modèle de vue courant est celui de la boîte
/// de dialogue ouverte (<see cref="FormDialog.Current"/>).
/// </summary>
public sealed partial class FormTemplates : ResourceDictionary
{
    public FormTemplates() => InitializeComponent();

    private static T? Model<T>() where T : FormViewModel => FormDialog.Current as T;

    // --- Compte ---

    private void OnClearAccountGroup(object sender, RoutedEventArgs args)
    {
        if (Model<AccountFormViewModel>() is { } model)
        {
            model.Group = null;
        }
    }

    // --- Groupes ---

    private void OnMoveGroupUp(object sender, RoutedEventArgs args) => MoveGroup(sender, -1);

    private void OnMoveGroupDown(object sender, RoutedEventArgs args) => MoveGroup(sender, 1);

    private void MoveGroup(object sender, int offset)
    {
        if (Model<AccountGroupsViewModel>() is { } model && sender is FrameworkElement { Tag: string group })
        {
            model.MoveCommand.Execute((group, offset));
        }
    }

    private async void OnRenameGroup(object sender, RoutedEventArgs args)
    {
        if (Model<AccountGroupsViewModel>() is not { } model || sender is not FrameworkElement { Tag: string group } element)
        {
            return;
        }
        var input = new TextBox { Text = group, PlaceholderText = "Nom du groupe" };
        var dialog = new ContentDialog
        {
            Title = "Renommer le groupe",
            Content = input,
            PrimaryButtonText = "Renommer",
            CloseButtonText = "Annuler",
            DefaultButton = ContentDialogButton.Primary,
            XamlRoot = element.XamlRoot,
        };
        if (await dialog.ShowAsync() == ContentDialogResult.Primary)
        {
            model.RenameCommand.Execute((group, input.Text));
        }
    }

    private void OnDeleteGroup(object sender, RoutedEventArgs args)
    {
        if (Model<AccountGroupsViewModel>() is { } model && sender is FrameworkElement { Tag: string group })
        {
            model.DeleteCommand.Execute(group);
        }
    }

    // --- Budget ---

    private void OnClearBudgetAccount(object sender, RoutedEventArgs args)
    {
        if (Model<BudgetFormViewModel>() is { } model)
        {
            model.AccountId = null;
        }
    }

    // --- Échéance ---

    private void OnClearScheduledBudget(object sender, RoutedEventArgs args)
    {
        if (Model<ScheduledFormViewModel>() is { } model)
        {
            model.BudgetId = null;
        }
    }

    // --- Suggestions ---

    private void OnAcceptBudgetSuggestion(object sender, RoutedEventArgs args)
    {
        if (Model<BudgetSuggestionsViewModel>() is { } model && sender is FrameworkElement { Tag: BudgetSuggestion suggestion })
        {
            model.AcceptCommand.Execute(suggestion);
        }
    }

    private void OnDismissBudgetSuggestion(object sender, RoutedEventArgs args)
    {
        if (Model<BudgetSuggestionsViewModel>() is { } model && sender is FrameworkElement { Tag: BudgetSuggestion suggestion })
        {
            model.DismissCommand.Execute(suggestion);
        }
    }

    private void OnAcceptScheduledSuggestion(object sender, RoutedEventArgs args)
    {
        if (Model<ScheduledSuggestionsViewModel>() is { } model && sender is FrameworkElement { Tag: ScheduledSuggestion suggestion })
        {
            model.AcceptCommand.Execute(suggestion);
        }
    }

    private void OnDismissScheduledSuggestion(object sender, RoutedEventArgs args)
    {
        if (Model<ScheduledSuggestionsViewModel>() is { } model && sender is FrameworkElement { Tag: ScheduledSuggestion suggestion })
        {
            model.DismissCommand.Execute(suggestion);
        }
    }

    // --- Restauration ---

    private void OnRestoreModeChecked(object sender, RoutedEventArgs args)
    {
        if (Model<RestoreBackupViewModel>() is { } model && sender is FrameworkElement { Tag: string tag })
        {
            model.Mode = tag == "merge" ? RestoreMode.Merge : RestoreMode.Replace;
        }
    }

    // --- Import de relevé ---

    private void OnSeparatorChecked(object sender, RoutedEventArgs args)
    {
        if (Model<StatementImportViewModel>() is { } model && sender is FrameworkElement { Tag: string tag })
        {
            model.Separator = tag == "tab" ? "\t" : tag;
        }
    }
}
