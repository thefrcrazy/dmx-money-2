using System.ComponentModel;
using DmxMoney.Interop;
using DmxMoney.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Navigation;

namespace DmxMoney.App;

public sealed partial class JournalPage : Page
{
    private sealed record FilterOption(string Id, string Label);

    public JournalViewModel ViewModel { get; private set; } = null!;

    private bool syncing;

    public JournalPage()
    {
        InitializeComponent();
        NavigationCacheMode = NavigationCacheMode.Required;
    }

    protected override void OnNavigatedTo(NavigationEventArgs args)
    {
        var shell = (ShellViewModel)args.Parameter;
        if (!ReferenceEquals(ViewModel, shell.Journal))
        {
            if (ViewModel is not null)
            {
                ViewModel.PropertyChanged -= OnViewModelChanged;
            }
            ViewModel = shell.Journal;
            ViewModel.PropertyChanged += OnViewModelChanged;
            syncing = true;
            CategoriesList.ItemsSource = shell.Store.Categories;
            TypesList.ItemsSource = JournalViewModel.TypeOptions.Select(option => new FilterOption(option.Id, option.Label)).ToList();
            StatusesList.ItemsSource = JournalViewModel.StatusOptions.Select(option => new FilterOption(option.Id, option.Label)).ToList();
            BudgetsList.ItemsSource = JournalViewModel.BudgetOptions.Select(option => new FilterOption(option.Id, option.Label)).ToList();
            syncing = false;
        }
        Bindings.Update();
        UpdateVisuals();
    }

    private void OnViewModelChanged(object? sender, PropertyChangedEventArgs args)
    {
        if (args.PropertyName is nameof(JournalViewModel.View) or nameof(JournalViewModel.Rows) or nameof(JournalViewModel.SelectionCount))
        {
            UpdateVisuals();
        }
    }

    private void UpdateVisuals()
    {
        var view = ViewModel.View;
        syncing = true;
        Rows.ItemsSource = ViewModel.Rows;
        syncing = false;

        var count = ViewModel.Rows.Count;
        var total = view?.TotalTransactionCount ?? 0;
        var net = view?.VisibleNet ?? 0;
        Summary.Text = $"{count} / {total} lignes · Net {(net >= 0 ? "+" : string.Empty)}{Money.Format(net)}";

        EmptyState.Visibility = count == 0 ? Visibility.Visible : Visibility.Collapsed;
        EmptyMessage.Text = ViewModel.HasFilters
            ? "Aucun résultat pour vos filtres actuels."
            : "Commencez par ajouter une transaction ou importez un relevé bancaire.";
        EmptyAction.Visibility = ViewModel.HasFilters ? Visibility.Collapsed : Visibility.Visible;

        SelectionBar.Visibility = ViewModel.SelectionCount > 0 ? Visibility.Visible : Visibility.Collapsed;
        SelectionLabel.Text = $"{ViewModel.SelectionCount} {(ViewModel.SelectionCount > 1 ? "sélectionnées" : "sélectionnée")}";

        CategoriesButton.Content = ViewModel.Categories.Count == 0 ? "Toutes les catégories" : $"Catégories ({ViewModel.Categories.Count})";
        TypesButton.Content = ViewModel.Types.Count == 0 ? "Tous les types" : $"Types ({ViewModel.Types.Count})";
        StatusesButton.Content = ViewModel.Statuses.Count == 0 ? "Tous les états" : $"États ({ViewModel.Statuses.Count})";
        BudgetsButton.Content = ViewModel.BudgetStatuses.Count == 0 ? "Tous les budgets" : $"Budgets ({ViewModel.BudgetStatuses.Count})";
    }

    private void OnCategoriesChanged(object sender, SelectionChangedEventArgs args)
    {
        if (!syncing)
        {
            ViewModel.Categories = [.. CategoriesList.SelectedItems.OfType<Category>().Select(category => category.Id)];
        }
    }

    private void OnTypesChanged(object sender, SelectionChangedEventArgs args)
    {
        if (!syncing)
        {
            ViewModel.Types = [.. TypesList.SelectedItems.OfType<FilterOption>().Select(option => option.Id)];
        }
    }

    private void OnStatusesChanged(object sender, SelectionChangedEventArgs args)
    {
        if (!syncing)
        {
            ViewModel.Statuses = [.. StatusesList.SelectedItems.OfType<FilterOption>().Select(option => option.Id)];
        }
    }

    private void OnBudgetsChanged(object sender, SelectionChangedEventArgs args)
    {
        if (!syncing)
        {
            ViewModel.BudgetStatuses = [.. BudgetsList.SelectedItems.OfType<FilterOption>().Select(option => option.Id)];
        }
    }

    private void OnSelectionChanged(object sender, SelectionChangedEventArgs args)
    {
        if (syncing)
        {
            return;
        }
        ViewModel.SetSelection(Rows.SelectedItems.OfType<JournalRow>().Select(row => row.Transaction.Id));
    }

    private void OnClearSelection(object sender, RoutedEventArgs args)
    {
        Rows.SelectedItems.Clear();
        ViewModel.SetSelection([]);
    }

    private void OnRowDoubleTapped(object sender, DoubleTappedRoutedEventArgs args)
    {
        // Les cellules éditables consomment déjà l'événement : ici on ouvre le formulaire.
        if (Rows.SelectedItems.OfType<JournalRow>().FirstOrDefault() is { } row)
        {
            ViewModel.EditCommand.Execute(row.Transaction.Id);
        }
    }

    private void OnToggleChecked(object sender, RoutedEventArgs args)
    {
        if (sender is FrameworkElement { Tag: string id })
        {
            ViewModel.ToggleCheckedCommand.Execute(id);
        }
    }

    private void OnEdit(object sender, RoutedEventArgs args)
    {
        if (sender is FrameworkElement { Tag: string id })
        {
            ViewModel.EditCommand.Execute(id);
        }
    }

    private void OnDelete(object sender, RoutedEventArgs args)
    {
        if (sender is FrameworkElement { Tag: string id })
        {
            ViewModel.DeleteCommand.Execute(id);
        }
    }
}
