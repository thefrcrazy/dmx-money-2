using System.ComponentModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using DmxMoney.Interop;

namespace DmxMoney.ViewModels;

/// <summary>Coquille de l'application : navigation, filtre global, soldes et présentation des formulaires.</summary>
public sealed partial class ShellViewModel : ObservableObject, IDisposable
{
    public EngineStore Store { get; }

    public DashboardViewModel Dashboard { get; }

    public AccountsViewModel Accounts { get; }

    public JournalViewModel Journal { get; }

    public BudgetViewModel Budget { get; }

    public ScheduledViewModel Scheduled { get; }

    public AnalyticsViewModel Analytics { get; }

    public PredictionsViewModel Predictions { get; }

    public CategoriesViewModel CategoriesPage { get; }

    public SettingsViewModel SettingsPage { get; }

    public ShellViewModel(EngineStore store)
    {
        Store = store;
        Dashboard = new DashboardViewModel(store);
        Accounts = new AccountsViewModel(store);
        Journal = new JournalViewModel(store);
        Budget = new BudgetViewModel(store);
        Scheduled = new ScheduledViewModel(store);
        Analytics = new AnalyticsViewModel(store);
        Predictions = new PredictionsViewModel(store);
        CategoriesPage = new CategoriesViewModel(store);
        SettingsPage = new SettingsViewModel(store);
        Store.PropertyChanged += OnStoreChanged;
        Activate();
    }

    public SidebarSection[] Sections => AppRoutes.SidebarSections;

    public AppRoute[] FooterRoutes => AppRoutes.FooterRoutes;

    public AppRoute Route
    {
        get => Store.Route;
        set => Store.Route = value;
    }

    public string RouteTitle => Store.Route.Title();

    public bool ShowsAccountFilter => Store.Route.UsesAccountFilter();

    public bool ShowsBalances => Store.Route.ShowsBalances();

    public PageViewModel CurrentPage => Store.Route switch
    {
        AppRoute.Dashboard => Dashboard,
        AppRoute.Accounts => Accounts,
        AppRoute.Transactions => Journal,
        AppRoute.Budget => Budget,
        AppRoute.Scheduled => Scheduled,
        AppRoute.Analytics => Analytics,
        AppRoute.Predictions => Predictions,
        AppRoute.Categories => CategoriesPage,
        _ => SettingsPage,
    };

    private IEnumerable<PageViewModel> Pages =>
    [
        Dashboard, Accounts, Journal, Budget, Scheduled, Analytics, Predictions, CategoriesPage, SettingsPage,
    ];

    /// <summary>Ouverture : échéances dues, pont PWA, nouveautés, reprise de la base 1.x.</summary>
    public void Start(string? pwaAssetsDirectory = null)
    {
        ReportLegacyImport();
        Store.ProcessDueScheduled();
        Store.StartBridge(pwaAssetsDirectory);
        PresentWhatsNewIfNeeded();
    }

    public void PresentWhatsNewIfNeeded()
    {
        var version = AppInfo.Version;
        if (Store.Settings.LastSeenVersion == version)
        {
            return;
        }
        if (Store.Settings.LastSeenVersion is null && Store.Accounts.Count == 0)
        {
            Store.Apply(new SettingsChange.SetLastSeenVersion(version));
            return;
        }
        Store.Present(new FormRequest.WhatsNew());
    }

    private void ReportLegacyImport()
    {
        var report = Store.Engine.OpenReport();
        if (report.LegacyImportError is { } error)
        {
            Store.ErrorMessage = $"La base DmxMoney 1.x n'a pas pu être reprise : {error}";
        }
        else if (report.ImportedLegacyDatabase is not null)
        {
            Store.ShowToast("Vos données DmxMoney 1.x ont été reprises");
        }
    }

    private void OnStoreChanged(object? sender, PropertyChangedEventArgs args)
    {
        switch (args.PropertyName)
        {
            case nameof(EngineStore.Route):
                Store.ProcessDueScheduled();
                Activate();
                OnPropertyChanged(nameof(Route));
                OnPropertyChanged(nameof(RouteTitle));
                OnPropertyChanged(nameof(ShowsAccountFilter));
                OnPropertyChanged(nameof(ShowsBalances));
                OnPropertyChanged(nameof(CurrentPage));
                break;
            case nameof(EngineStore.Revision):
                OnPropertyChanged(nameof(AccountFilterLabel));
                break;
        }
    }

    private void Activate()
    {
        var current = CurrentPage;
        foreach (var page in Pages)
        {
            page.IsActive = ReferenceEquals(page, current);
        }
    }

    /// <summary>Libellé du bouton de filtre : « Tous les comptes », le nom du compte, ou le décompte.</summary>
    public string AccountFilterLabel => Store.SelectedAccountIds.Count switch
    {
        0 => "Tous les comptes",
        1 => Store.AccountById(Store.SelectedAccountIds[0])?.Name ?? "1 compte",
        var count => $"{count} comptes",
    };

    [RelayCommand]
    private void Navigate(AppRoute route) => Store.Route = route;

    [RelayCommand]
    private void NewTransaction() => Store.Present(new FormRequest.TransactionForm(null));

    [RelayCommand]
    private void ToggleAccountFilter(string accountId) => Store.ToggleAccountFilter(accountId);

    [RelayCommand]
    private void ClearAccountFilter() => Store.ClearAccountFilter();

    [RelayCommand]
    private void Sync()
    {
        Store.Reload();
        Store.RefreshBridgeStatus();
    }

    public void Dispose()
    {
        Store.PropertyChanged -= OnStoreChanged;
        foreach (var page in Pages)
        {
            page.Dispose();
        }
    }
}
