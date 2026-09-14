using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using DmxMoney.Interop;

namespace DmxMoney.ViewModels;

public sealed partial class DashboardViewModel : PageViewModel
{
    [ObservableProperty]
    private DashboardView? view;

    public DashboardViewModel(EngineStore store) : base(store) => Refresh();

    public override void Refresh()
    {
        var accounts = Store.SelectedAccountIds.ToArray();
        var today = Store.Today;
        View = Store.Read(engine => engine.Dashboard(accounts, today));
    }

    [RelayCommand]
    private void NewAccount() => Store.Present(new FormRequest.AccountForm(null));

    [RelayCommand]
    private void OpenScheduled() => Store.Route = AppRoute.Scheduled;

    [RelayCommand]
    private void OpenJournal() => Store.Route = AppRoute.Transactions;

    /// <summary>Clic sur un compte : filtre sur ce compte et ouvre le journal.</summary>
    [RelayCommand]
    private void OpenAccount(string accountId)
    {
        Store.SelectedAccountIds = [accountId];
        Store.Route = AppRoute.Transactions;
    }
}
