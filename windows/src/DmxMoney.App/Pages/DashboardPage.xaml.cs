using System.ComponentModel;
using DmxMoney.Interop;
using DmxMoney.ViewModels;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;

namespace DmxMoney.App;

public sealed partial class DashboardPage : Page
{
    public DashboardViewModel ViewModel { get; private set; } = null!;

    private ShellViewModel shell = null!;

    public DashboardPage()
    {
        InitializeComponent();
        NavigationCacheMode = NavigationCacheMode.Required;
    }

    protected override void OnNavigatedTo(NavigationEventArgs args)
    {
        shell = (ShellViewModel)args.Parameter;
        if (!ReferenceEquals(ViewModel, shell.Dashboard))
        {
            if (ViewModel is not null)
            {
                ViewModel.PropertyChanged -= OnViewModelChanged;
            }
            ViewModel = shell.Dashboard;
            ViewModel.PropertyChanged += OnViewModelChanged;
        }
        Bindings?.Update();
        UpdateVisuals();
    }

    private void OnViewModelChanged(object? sender, PropertyChangedEventArgs args)
    {
        if (args.PropertyName == nameof(DashboardViewModel.View))
        {
            Bindings?.Update();
            UpdateVisuals();
        }
    }

    private void UpdateVisuals()
    {
        var view = ViewModel.View;
        if (view is null)
        {
            return;
        }
        TopCategories.ItemsSource = view.TopCategories.Take(3).ToList();
        AccountsList.ItemsSource = view.Accounts;
        RecentList.ItemsSource = view.RecentTransactions;
        BudgetDonut.Update(
        [
            new DonutSliceData("spent", "Dépenses", view.Budget.Spent, view.Budget.Remaining >= 0 ? "#10b981" : "#ef4444"),
            new DonutSliceData("remaining", "Restant", Math.Max(view.Budget.Remaining, 0), "#33808080"),
        ],
            "Dépenses",
            Format.PercentTight(view.Budget.Progress));
    }

    private void OnAccountClicked(object sender, ItemClickEventArgs args)
    {
        if (args.ClickedItem is DashboardAccount account)
        {
            ViewModel.OpenAccountCommand.Execute(account.Account.Id);
        }
    }
}
