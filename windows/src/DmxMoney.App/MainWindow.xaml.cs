using System.ComponentModel;
using System.Windows.Input;
using CommunityToolkit.Mvvm.Input;
using DmxMoney.Interop;
using DmxMoney.ViewModels;
using Microsoft.UI;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Windows.Graphics;
using Windows.UI;

namespace DmxMoney.App;

/// <summary>Compte proposé dans le filtre global.</summary>
public sealed record AccountFilterItem(string Id, string Name, string Icon, Brush Brush);

public sealed partial class MainWindow : Window
{
    private static readonly Dictionary<AppRoute, string> NavigationGlyphs = new()
    {
        [AppRoute.Dashboard] = "LayoutDashboard",
        [AppRoute.Accounts] = "Wallet",
        [AppRoute.Transactions] = "Receipt",
        [AppRoute.Budget] = "Calculator",
        [AppRoute.Scheduled] = "CalendarClock",
        [AppRoute.Analytics] = "PieChart",
        [AppRoute.Predictions] = "TrendingUp",
        [AppRoute.Categories] = "Tag",
        [AppRoute.Settings] = "Settings",
    };

    private static readonly Dictionary<AppRoute, Type> PageTypes = new()
    {
        [AppRoute.Dashboard] = typeof(DashboardPage),
        [AppRoute.Accounts] = typeof(AccountsPage),
        [AppRoute.Transactions] = typeof(JournalPage),
        [AppRoute.Budget] = typeof(BudgetPage),
        [AppRoute.Scheduled] = typeof(ScheduledPage),
        [AppRoute.Analytics] = typeof(AnalyticsPage),
        [AppRoute.Predictions] = typeof(PredictionsPage),
        [AppRoute.Categories] = typeof(CategoriesPage),
        [AppRoute.Settings] = typeof(SettingsPage),
    };

    private readonly Dictionary<AppRoute, NavigationViewItem> navigationItems = [];
    private ShellViewModel? shell;
    private ContentDialog? openDialog;
    private bool syncingFilter;
    private bool alerting;

    public ICommand ShowWindowCommand { get; }

    public MainWindow()
    {
        ShowWindowCommand = new RelayCommand(ShowFromTray);
        InitializeComponent();
        SystemBackdrop = new MicaBackdrop();
        VersionLabel.Text = $"DMXMONEY • V{AppInfo.Version}";

        if (WindowState.Load() is { } frame)
        {
            AppWindow.MoveAndResize(new RectInt32(frame.X, frame.Y, frame.Width, frame.Height));
        }
        else
        {
            AppWindow.Resize(new SizeInt32(1320, 860));
        }
        AppWindow.Closing += OnClosing;
        AppWindow.Changed += OnAppWindowChanged;
    }

    public void Attach(ShellViewModel viewModel)
    {
        shell = viewModel;
        BuildNavigation();
        BuildTrayMenu();
        viewModel.Store.PropertyChanged += OnStoreChanged;
        ApplyTheme();
        ApplyAccent();
        UpdateAccountFilter();
        UpdateHeader();
        Navigate(viewModel.Route);
        if (Environment.GetEnvironmentVariable("DMXMONEY_SELF_TEST") == "1")
        {
            StartSelfTest();
        }
    }

    // --- Auto-test (CI) ---

    private DispatcherQueueTimer? selfTestTimer;

    /// <summary>Ouvre chaque page tour à tour puis quitte (code 0) : la CI y détecte une page qui plante.</summary>
    private void StartSelfTest()
    {
        var routes = Enum.GetValues<AppRoute>();
        var index = 0;
        selfTestTimer = DispatcherQueue.CreateTimer();
        selfTestTimer.Interval = TimeSpan.FromMilliseconds(1500);
        selfTestTimer.Tick += (timer, args) =>
        {
            if (index < routes.Length && shell is not null)
            {
                shell.Route = routes[index++];
                return;
            }
            timer.Stop();
            CrashReport.Note("auto-test terminé");
            Environment.Exit(0);
        };
        selfTestTimer.Start();
    }

    // --- Navigation ---

    private void BuildNavigation()
    {
        Nav.MenuItems.Clear();
        foreach (var section in AppRoutes.SidebarSections)
        {
            Nav.MenuItems.Add(new NavigationViewItemHeader { Content = section.Title });
            foreach (var route in section.Routes)
            {
                Nav.MenuItems.Add(CreateItem(route));
            }
        }
        Nav.FooterMenuItems.Clear();
        foreach (var route in AppRoutes.FooterRoutes)
        {
            Nav.FooterMenuItems.Add(CreateItem(route));
        }
    }

    private NavigationViewItem CreateItem(AppRoute route)
    {
        var content = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 12 };
        content.Children.Add(new LucideIcon { Glyph = NavigationGlyphs[route], Size = 16 });
        content.Children.Add(new TextBlock { Text = route.Title() });
        var item = new NavigationViewItem { Content = content, Tag = route };
        navigationItems[route] = item;
        return item;
    }

    private void OnNavigationItemInvoked(NavigationView sender, NavigationViewItemInvokedEventArgs args)
    {
        if (args.InvokedItemContainer?.Tag is AppRoute route && shell is not null)
        {
            shell.Route = route;
        }
    }

    private void Navigate(AppRoute route)
    {
        if (shell is null)
        {
            return;
        }
        CrashReport.Note($"page {route}");
        if (navigationItems.TryGetValue(route, out var item))
        {
            Nav.SelectedItem = item;
        }
        if (ContentFrame.Content?.GetType() != PageTypes[route])
        {
            ContentFrame.Navigate(PageTypes[route], shell, new Microsoft.UI.Xaml.Media.Animation.SuppressNavigationTransitionInfo());
        }
        FilterLabel.Visibility = AccountFilterButton.Visibility = route.UsesAccountFilter() ? Visibility.Visible : Visibility.Collapsed;
        BalancePanel.Visibility = route.ShowsBalances() ? Visibility.Visible : Visibility.Collapsed;
    }

    // --- Réactions au store ---

    private void OnStoreChanged(object? sender, PropertyChangedEventArgs args)
    {
        if (shell is null)
        {
            return;
        }
        switch (args.PropertyName)
        {
            case nameof(EngineStore.Route):
                Navigate(shell.Route);
                break;
            case nameof(EngineStore.Settings):
                ApplyTheme();
                ApplyAccent();
                break;
            case nameof(EngineStore.Revision):
                UpdateAccountFilter();
                UpdateHeader();
                break;
            case nameof(EngineStore.Form):
                PresentForm(shell.Store.Form);
                break;
            case nameof(EngineStore.Confirmation):
                if (shell.Store.Confirmation is { } confirmation)
                {
                    PresentConfirmation(confirmation);
                }
                break;
            case nameof(EngineStore.ErrorMessage):
                if (shell.Store.ErrorMessage is { } message)
                {
                    PresentError(message);
                }
                break;
            case nameof(EngineStore.Toast):
                Toast.Message = shell.Store.Toast ?? string.Empty;
                Toast.IsOpen = shell.Store.Toast is not null;
                break;
            case nameof(EngineStore.DueResult):
                if (shell.Store.DueResult is { CreatedTransactions: > 0 } due)
                {
                    Toast.Message = $"{Plural.Of((int)due.CreatedTransactions, "échéance")} ajoutée au journal";
                    Toast.IsOpen = true;
                }
                break;
        }
    }

    private void UpdateHeader()
    {
        if (shell is null)
        {
            return;
        }
        AccountFilterButton.Content = shell.AccountFilterLabel;
        CheckedBalance.Text = Money.Format(shell.Store.Balances.CheckedBalance);
        CurrentBalance.Text = Money.Format(shell.Store.Balances.CurrentBalance);
    }

    private void UpdateAccountFilter()
    {
        if (shell is null)
        {
            return;
        }
        syncingFilter = true;
        var items = shell.Store.Accounts
            .Select(account => new AccountFilterItem(account.Id, account.Name, account.Icon, Palette.ToBrush(account.Color, Colors.SteelBlue)))
            .ToList();
        AccountFilterList.ItemsSource = items;
        AccountFilterList.SelectedItems.Clear();
        foreach (var item in items.Where(item => shell.Store.SelectedAccountIds.Contains(item.Id)))
        {
            AccountFilterList.SelectedItems.Add(item);
        }
        syncingFilter = false;
    }

    private void OnAccountFilterChanged(object sender, SelectionChangedEventArgs args)
    {
        if (syncingFilter || shell is null)
        {
            return;
        }
        var selection = AccountFilterList.SelectedItems.OfType<AccountFilterItem>().Select(item => item.Id).ToList();
        shell.Store.SelectedAccountIds = selection.Count == shell.Store.Accounts.Count ? [] : selection;
        UpdateHeader();
    }

    private void OnClearAccountFilter(object sender, RoutedEventArgs args)
    {
        shell?.Store.ClearAccountFilter();
        UpdateAccountFilter();
        UpdateHeader();
    }

    // --- Apparence ---

    private void ApplyTheme()
    {
        Root.RequestedTheme = shell?.Store.Settings.Theme switch
        {
            Theme.Light => ElementTheme.Light,
            Theme.Dark => ElementTheme.Dark,
            _ => ElementTheme.Default,
        };
    }

    private void ApplyAccent()
    {
        var hex = shell?.Store.Settings.AccentColor;
        if (hex is null)
        {
            Root.Resources.Remove("AccentFillColorDefaultBrush");
            Root.Resources.Remove("AccentFillColorSecondaryBrush");
            Root.Resources.Remove("AccentFillColorTertiaryBrush");
            Root.Resources.Remove("DmxAccent");
            return;
        }
        var color = Palette.ToColor(hex, Colors.SlateBlue);
        Root.Resources["AccentFillColorDefaultBrush"] = new SolidColorBrush(color);
        Root.Resources["AccentFillColorSecondaryBrush"] = new SolidColorBrush(Color.FromArgb(230, color.R, color.G, color.B));
        Root.Resources["AccentFillColorTertiaryBrush"] = new SolidColorBrush(Color.FromArgb(200, color.R, color.G, color.B));
        Root.Resources["DmxAccent"] = new SolidColorBrush(color);
    }

    // --- Boîtes de dialogue ---

    private async void PresentForm(FormRequest? request)
    {
        if (request is null || shell is null)
        {
            return;
        }
        CloseOpenDialog();
        CrashReport.Note($"formulaire {request.GetType().Name}");
        var model = FormFactory.Create(shell.Store, request);
        if (model is null)
        {
            shell.Store.Form = null;
            return;
        }
        var dialog = new FormDialog(model) { XamlRoot = Root.XamlRoot };
        openDialog = dialog;
        await dialog.ShowAsync();
        openDialog = null;
        if (ReferenceEquals(shell.Store.Form, request))
        {
            shell.Store.Form = null;
        }
    }

    private async void PresentConfirmation(ConfirmRequest request)
    {
        if (shell is null)
        {
            return;
        }
        shell.Store.Confirmation = null;
        CloseOpenDialog();
        var dialog = new ContentDialog
        {
            Title = request.Title,
            Content = new TextBlock { Text = request.Message, TextWrapping = TextWrapping.Wrap },
            PrimaryButtonText = request.ConfirmTitle,
            CloseButtonText = "Annuler",
            DefaultButton = ContentDialogButton.Close,
            XamlRoot = Root.XamlRoot,
        };
        openDialog = dialog;
        var result = await dialog.ShowAsync();
        openDialog = null;
        if (result == ContentDialogResult.Primary)
        {
            request.Action();
        }
    }

    private async void PresentError(string message)
    {
        if (shell is null || alerting)
        {
            return;
        }
        alerting = true;
        shell.Store.ErrorMessage = null;
        CloseOpenDialog();
        var dialog = new ContentDialog
        {
            Title = "DmxMoney",
            Content = new TextBlock { Text = message, TextWrapping = TextWrapping.Wrap },
            CloseButtonText = "OK",
            XamlRoot = Root.XamlRoot,
        };
        openDialog = dialog;
        await dialog.ShowAsync();
        openDialog = null;
        alerting = false;
    }

    private void CloseOpenDialog()
    {
        if (openDialog is not null)
        {
            openDialog.Hide();
            openDialog = null;
        }
    }

    // --- Barre des tâches ---

    private void BuildTrayMenu()
    {
        var flyout = new MenuFlyout();
        flyout.Opening += (_, _) => FillTrayMenu(flyout);
        Tray.ContextFlyout = flyout;
    }

    private void FillTrayMenu(MenuFlyout flyout)
    {
        flyout.Items.Clear();
        if (shell is null)
        {
            return;
        }
        if (shell.Store.Read(engine => engine.TraySummary()) is { } summary)
        {
            foreach (var account in summary.Accounts.Take(8))
            {
                var item = new MenuFlyoutItem { Text = $"{account.Name} — {Money.Format(account.Balance)}" };
                item.Click += (_, _) =>
                {
                    shell.Store.SelectedAccountIds = [account.AccountId];
                    shell.Store.Route = AppRoute.Transactions;
                    ShowFromTray();
                };
                flyout.Items.Add(item);
            }
            if (summary.Accounts.Length > 8)
            {
                flyout.Items.Add(new MenuFlyoutItem { Text = $"… et {summary.Accounts.Length - 8} autres comptes", IsEnabled = false });
            }
            flyout.Items.Add(new MenuFlyoutItem { Text = $"Total — {Money.Format(summary.Total)}", IsEnabled = false });
            flyout.Items.Add(new MenuFlyoutSeparator());
        }

        var open = new MenuFlyoutItem { Text = "Ouvrir DmxMoney" };
        open.Click += (_, _) => ShowFromTray();
        flyout.Items.Add(open);

        var add = new MenuFlyoutItem { Text = "Nouvelle transaction…" };
        add.Click += (_, _) =>
        {
            ShowFromTray();
            shell.Store.Present(new FormRequest.TransactionForm(null));
        };
        flyout.Items.Add(add);
        flyout.Items.Add(new MenuFlyoutSeparator());

        foreach (var route in new[] { AppRoute.Dashboard, AppRoute.Transactions, AppRoute.Budget, AppRoute.Scheduled, AppRoute.Predictions })
        {
            var item = new MenuFlyoutItem { Text = route.Title() };
            item.Click += (_, _) =>
            {
                shell.Store.Route = route;
                ShowFromTray();
            };
            flyout.Items.Add(item);
        }
        flyout.Items.Add(new MenuFlyoutSeparator());

        var sync = new MenuFlyoutItem { Text = "Synchroniser" };
        sync.Click += (_, _) => shell.SyncCommand.Execute(null);
        flyout.Items.Add(sync);

        var quit = new MenuFlyoutItem { Text = "Quitter DmxMoney" };
        quit.Click += (_, _) => Quit();
        flyout.Items.Add(quit);
    }

    private void ShowFromTray()
    {
        AppWindow.Show();
        Activate();
    }

    private void OnQuit(object sender, RoutedEventArgs args) => Quit();

    private void Quit()
    {
        Tray.Dispose();
        // Le shell libère les pages (abonnements au store) avant le moteur lui-même.
        shell?.Dispose();
        shell?.Store.Dispose();
        Application.Current.Exit();
    }

    // --- Fenêtre ---

    private void OnClosing(AppWindow sender, AppWindowClosingEventArgs args)
    {
        // Fermer masque la fenêtre : le pont PWA et l'icône de la barre des tâches restent actifs.
        args.Cancel = true;
        sender.Hide();
    }

    private void OnAppWindowChanged(AppWindow sender, AppWindowChangedEventArgs args)
    {
        if (args.DidSizeChange || args.DidPositionChange)
        {
            WindowState.Save(sender.Size.Width, sender.Size.Height, sender.Position.X, sender.Position.Y);
        }
    }
}
