using System.ComponentModel;
using DmxMoney.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;

namespace DmxMoney.App;

public sealed partial class AccountsPage : Page
{
    public AccountsViewModel ViewModel { get; private set; } = null!;

    private bool syncingTypes;

    public AccountsPage()
    {
        InitializeComponent();
        NavigationCacheMode = NavigationCacheMode.Required;
    }

    protected override void OnNavigatedTo(NavigationEventArgs args)
    {
        var shell = (ShellViewModel)args.Parameter;
        if (!ReferenceEquals(ViewModel, shell.Accounts))
        {
            if (ViewModel is not null)
            {
                ViewModel.PropertyChanged -= OnViewModelChanged;
            }
            ViewModel = shell.Accounts;
            ViewModel.PropertyChanged += OnViewModelChanged;
            syncingTypes = true;
            TypesList.ItemsSource = ViewModel.AccountTypes;
            syncingTypes = false;
        }
        Bindings.Update();
        UpdateVisuals();
    }

    private void OnViewModelChanged(object? sender, PropertyChangedEventArgs args)
    {
        if (args.PropertyName is nameof(AccountsViewModel.View))
        {
            UpdateVisuals();
        }
    }

    private void UpdateVisuals()
    {
        var view = ViewModel.View;
        Groups.ItemsSource = view?.Groups;
        var total = view?.TotalCount ?? 0;
        var visible = view?.VisibleCount ?? 0;
        EmptyState.Visibility = visible == 0 ? Visibility.Visible : Visibility.Collapsed;
        EmptyTitle.Text = total == 0 ? "Aucun compte configuré" : "Aucun compte ne correspond aux filtres.";
        EmptyMessage.Text = total == 0 ? string.Empty : "Modifie la recherche ou les types sélectionnés.";
        EmptyAction.Visibility = total == 0 ? Visibility.Visible : Visibility.Collapsed;
        TypesButton.Content = ViewModel.Types.Count == 0 ? "Tous les types" : string.Join(", ", ViewModel.Types);
    }

    private void OnTypesChanged(object sender, SelectionChangedEventArgs args)
    {
        if (syncingTypes)
        {
            return;
        }
        ViewModel.Types = [.. TypesList.SelectedItems.OfType<string>()];
        UpdateVisuals();
    }

    private void OnEditAccount(object sender, RoutedEventArgs args)
    {
        if (sender is FrameworkElement { Tag: string id })
        {
            ViewModel.EditAccountCommand.Execute(id);
        }
    }

    private void OnDeleteAccount(object sender, RoutedEventArgs args)
    {
        if (sender is FrameworkElement { Tag: string id })
        {
            ViewModel.DeleteAccountCommand.Execute(id);
        }
    }
}
