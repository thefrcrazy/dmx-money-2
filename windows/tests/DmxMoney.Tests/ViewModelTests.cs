using System.Globalization;
using System.Text;
using DmxMoney.Interop;
using DmxMoney.ViewModels;

namespace DmxMoney.Tests;

public class ViewModelTests
{
    private const string Today = "2026-09-11";

    private static EngineStore NewStore() => new(DmxEngine.OpenInMemory());

    private static string AddAccount(EngineStore store, string name = "Compte courant", double balance = 1000)
    {
        var draft = store.Engine.AccountDraft(null) with { Name = name, InitialBalance = balance };
        return store.Engine.SaveAccount(draft);
    }

    private static void AddTransaction(EngineStore store, string accountId, double amount, string description, TransactionType kind = TransactionType.Expense, string category = "28")
    {
        var draft = store.Engine.TransactionDraft(null, [], Today) with
        {
            Kind = kind,
            Amount = amount,
            Description = description,
            AccountId = accountId,
            CategoryId = category,
        };
        store.Engine.SaveTransaction(draft);
    }

    [Fact]
    public void StoreExposesAccountsCategoriesAndBalances()
    {
        using var store = NewStore();
        Assert.Contains(store.Categories, category => category.Id == "transfer");

        var accountId = AddAccount(store);
        AddTransaction(store, accountId, 45.5, "Courses");
        store.Reload();

        Assert.Equal("Compte courant", Assert.Single(store.Accounts).Name);
        Assert.Equal(954.5, store.Balances.CurrentBalance, 3);
        Assert.False(store.IsFiltering);

        store.ToggleAccountFilter(accountId);
        Assert.True(store.IsSelected(accountId));
    }

    [Fact]
    public void DashboardViewModelFollowsTheAccountFilter()
    {
        using var store = NewStore();
        var first = AddAccount(store, "Courant", 100);
        var second = AddAccount(store, "Livret", 500);
        store.Reload();

        var dashboard = new DashboardViewModel(store);
        Assert.Equal(600, dashboard.View!.AccountsTotal, 3);

        store.SelectedAccountIds = [second];
        Assert.Equal(500, dashboard.View!.AccountsTotal, 3);
        Assert.Equal("Livret", Assert.Single(dashboard.View!.Accounts).Account.Name);
        Assert.NotEqual(first, second);
    }

    [Fact]
    public void JournalViewModelFiltersAndPointsTransactions()
    {
        using var store = NewStore();
        var accountId = AddAccount(store);
        AddTransaction(store, accountId, 12, "Boulangerie");
        AddTransaction(store, accountId, 2500, "Salaire", TransactionType.Income, "1");
        store.Reload();

        var journal = new JournalViewModel(store);
        Assert.Equal(2, journal.Rows.Count);

        journal.Search = "boulang";
        Assert.Equal("Boulangerie", Assert.Single(journal.Rows).Transaction.Description);
        Assert.True(journal.HasFilters);

        journal.Search = string.Empty;
        journal.Types = ["income"];
        Assert.Equal("Salaire", Assert.Single(journal.Rows).Transaction.Description);

        journal.Types = [];
        var id = journal.Rows.First(row => row.Transaction.Description == "Boulangerie").Transaction.Id;
        journal.ToggleCheckedCommand.Execute(id);
        Assert.True(journal.Rows.First(row => row.Transaction.Id == id).Transaction.Checked);

        Assert.True(journal.UpdateAmount(id, "15,50"));
        Assert.Equal(15.5, journal.Rows.First(row => row.Transaction.Id == id).Transaction.Amount, 3);
        Assert.False(journal.UpdateAmount(id, "abc"));
    }

    [Fact]
    public void JournalClearsFiltersOnceAndDefersHiddenChanges()
    {
        using var store = NewStore();
        using var journal = new JournalViewModel(store);
        journal.Search = "test";
        journal.Categories = ["5"];
        journal.Types = ["expense"];
        journal.Statuses = ["unchecked"];
        journal.BudgetStatuses = ["budgeted"];
        var refreshes = 0;
        journal.PropertyChanged += (_, args) =>
        {
            if (args.PropertyName == nameof(JournalViewModel.Rows)) { refreshes++; }
        };
        journal.ClearFilters();
        Assert.Equal(1, refreshes);
        Assert.False(journal.HasFilters);
        journal.ClearFilters();
        Assert.Equal(1, refreshes);

        journal.IsActive = false;
        journal.Search = "nouvelle recherche";
        journal.Types = ["income"];
        Assert.Equal(1, refreshes);
        journal.IsActive = true;
        Assert.Equal(2, refreshes);
        Assert.True(journal.HasFilters);
    }

    [Fact]
    public void JournalSelectionDoesNotNotifyWhenTheSameIdsAreSelectedAgain()
    {
        using var store = NewStore();
        using var journal = new JournalViewModel(store);
        var notifications = 0;
        journal.PropertyChanged += (_, args) =>
        {
            if (args.PropertyName == nameof(JournalViewModel.SelectionCount)) { notifications++; }
        };
        journal.SetSelection(["a", "b"]);
        journal.SetSelection(["b", "a", "a"]);
        Assert.Equal(1, notifications);
        journal.SetSelection([]);
        Assert.Equal(2, notifications);
    }

    [Fact]
    public async Task AsyncStatusReadsKeepJournalRowsAndSelectionWhenDataHasNotChanged()
    {
        using var store = NewStore();
        var account = AddAccount(store);
        AddTransaction(store, account, 12, "Boulangerie");
        store.Reload();
        using var journal = new JournalViewModel(store);
        var rows = journal.Rows;
        var selectedId = rows[0].Transaction.Id;
        journal.SetSelection([selectedId]);
        var revision = store.Revision;
        var callbacks = 0;
        for (var read = 0; read < 16; read++)
        {
            await store.PerformAsync(engine => engine.DataVersion(), _ => callbacks++);
        }
        Assert.Equal(16, callbacks);
        Assert.Equal(revision, store.Revision);
        Assert.Same(rows, journal.Rows);
        Assert.Equal(selectedId, Assert.Single(journal.Selection));

        // A real write still refreshes the presentation and keeps selection by ID.
        await store.PerformAsync(engine =>
        {
            engine.UpdateTransactionDescription(selectedId, "Pain");
            return true;
        });
        Assert.True(store.Revision > revision);
        Assert.Equal("Pain", Assert.Single(journal.Rows).Transaction.Description);
        Assert.Equal(selectedId, Assert.Single(journal.Selection));
    }

    [Fact]
    public async Task AsyncNoOpStillRefreshesDateSensitivePagesAfterMidnight()
    {
        var today = Today;
        using var store = new EngineStore(DmxEngine.OpenInMemory(), todayProvider: () => today);
        var revision = store.Revision;
        var version = store.DataVersion;
        today = "2026-09-12";
        await store.PerformAsync(engine => engine.DataVersion());
        Assert.Equal(version, store.DataVersion);
        Assert.Equal(revision + 1, store.Revision);
        await store.PerformAsync(engine => engine.DataVersion());
        Assert.Equal(revision + 1, store.Revision);
    }

    [Fact]
    public async Task AsyncSettingsWritesStillPublishUpdatedSettings()
    {
        using var store = NewStore();
        var revision = store.Revision;
        await store.PerformAsync(engine =>
        {
            engine.ApplySettingsChange(new SettingsChange.AddCustomGroup("Investissements"));
            return true;
        });
        Assert.Contains("Investissements", store.Settings.CustomGroups);
        Assert.True(store.Revision > revision);
    }

    [Fact]
    public void AccountFormRejectsInvalidAmountAndSavesGroup()
    {
        using var store = NewStore();
        store.Run(engine => engine.ApplySettingsChange(new SettingsChange.AddCustomGroup("Épargne")));

        var form = new AccountFormViewModel(store, store.Engine.AccountDraft(null));
        form.Name = "Livret A";
        form.AmountText = "abc";
        Assert.False(form.Submit());
        Assert.Equal("Saisissez un montant valide", form.Error);

        form.AmountText = "1 200,50";
        form.Group = "Épargne";
        Assert.True(form.Submit());
        Assert.Equal(1200.5, Assert.Single(store.Accounts).InitialBalance, 3);
        Assert.Equal("Épargne", store.Settings.AccountGroups[store.Accounts[0].Id]);
    }

    [Fact]
    public void AccountFormTypeChangePicksCoreDefaults()
    {
        using var store = NewStore();
        var form = new AccountFormViewModel(store, store.Engine.AccountDraft(null));
        form.AccountType = "Épargne";
        var defaults = DmxFfiMethods.AccountTypeDefaults("Épargne");
        Assert.Equal(defaults.Icon, form.Icon);
        Assert.Equal(defaults.Color, form.Color);
    }

    [Fact]
    public void ScheduledFormLinkedBudgetForcesCategoryAndAccount()
    {
        using var store = NewStore();
        var accountId = AddAccount(store);
        var budgetId = store.Engine.SaveBudget(new BudgetDraft(null, "Courses", 300, "5", accountId));
        store.Reload();

        var form = new ScheduledFormViewModel(store, store.Engine.ScheduledDraft(null, Today));
        form.Kind = TransactionType.Expense;
        form.Description = "Courses hebdo";
        form.AmountText = "80";
        form.BudgetId = budgetId;

        Assert.Equal("5", form.CategoryId);
        Assert.Equal(accountId, form.AccountId);
        Assert.True(form.Submit());

        var scheduled = Assert.Single(store.Read(engine => engine.Scheduled(
            new ScheduledQuery([], ScheduledDueRange.All, string.Empty, [], []), Today))!.Rows);
        Assert.Equal(budgetId, scheduled.Scheduled.BudgetId);
    }

    [Fact]
    public void CategoriesSearchIgnoresAccents()
    {
        using var store = NewStore();
        var accented = store.Categories.First(category =>
            category.Name.Any(character => "éèêëàâôûüîïçÉÈÀÔÇ".Contains(character)));
        var withoutAccents = string.Concat(accented.Name.Normalize(NormalizationForm.FormD)
            .Where(character => CharUnicodeInfo.GetUnicodeCategory(character) != UnicodeCategory.NonSpacingMark));

        var categories = new CategoriesViewModel(store);
        categories.Search = withoutAccents.ToLowerInvariant();
        Assert.Contains(categories.Visible, category => category.Id == accented.Id);

        categories.Search = "zzz";
        Assert.Empty(categories.Visible);
    }

    [Fact]
    public async Task StatementImportWizardImportsCsvIntoANewAccount()
    {
        using var store = NewStore();
        const string csv = "date;montant;libelle;categorie\n01/09/2026;-12,50;Boulangerie;Alimentation\n02/09/2026;1500,00;Salaire;Salaire\n";
        var wizard = new StatementImportViewModel(store, csv, "releve.csv");

        await wizard.PreparationTask;
        Assert.Equal(StatementFormat.Csv, wizard.Format);
        Assert.NotNull(wizard.Preview);
        wizard.SetRole(2, "description");
        wizard.SetRole(3, "category");
        Assert.True(wizard.CanContinue);

        await wizard.NextCommand.ExecuteAsync(null);
        Assert.Equal(StatementImportViewModel.Step.Account, wizard.CurrentStep);
        Assert.Equal(2, wizard.Transactions.Count);

        wizard.AccountChoice = StatementImportViewModel.NewAccountId;
        wizard.NewAccountName = "Compte importé";
        wizard.FinalBalanceText = "1 487,50";
        await wizard.NextCommand.ExecuteAsync(null);
        Assert.Equal(StatementImportViewModel.Step.Categories, wizard.CurrentStep);
        Assert.Equal(2, wizard.Sources.Count);

        await wizard.NextCommand.ExecuteAsync(null);
        Assert.Equal(StatementImportViewModel.Step.Confirm, wizard.CurrentStep);
        Assert.Equal(0, wizard.ComputedInitialBalance!.Value, 3);

        Assert.True(await wizard.SubmitAsync());
        await wizard.ImportTask!;

        var account = Assert.Single(store.Accounts);
        Assert.Equal("Compte importé", account.Name);
        Assert.Equal(1487.5, store.Balances.CurrentBalance, 3);
    }

    [Theory]
    [InlineData(null)]
    [InlineData("")]
    [InlineData("bad")]
    [InlineData("2026-13-50")]
    public void BackupDateHandlesMissingAndMalformedTimestamp(string? timestamp)
        => Assert.Null(RestoreBackupViewModel.FormatBackupDate(timestamp));

    [Fact]
    public async Task BackupWithoutTimestampCanBeInspectedWithoutCrashingTheForm()
    {
        using var store = NewStore();
        var form = new RestoreBackupViewModel(store, "{\"version\":1,\"data\":{}}", "fixture.dmx");
        await form.PreparationTask;
        Assert.NotNull(form.Summary);
        Assert.Null(form.BackupDate);
        Assert.Null(form.Error);
        Assert.NotNull(RestoreBackupViewModel.FormatBackupDate("2026-10-03T00:00:00Z"));
    }

    [Fact]
    public async Task FailedImportKeepsDraftAndVisibleErrorAndCanBeRetried()
    {
        using var store = NewStore();
        var id = AddAccount(store);
        store.Reload();
        var form = new StatementImportViewModel(store, "date;montant;libelle\n01/09/2026;-12,50;Fixture\n", "fixture.csv");
        await form.PreparationTask;
        form.SetRole(2, "description");
        await form.SubmitAsync();
        form.AccountChoice = "removed-account";
        await form.SubmitAsync();
        Assert.Equal(StatementImportViewModel.Step.Confirm, form.CurrentStep);
        Assert.False(await form.SubmitAsync());
        Assert.NotNull(form.Error);
        Assert.False(form.Importing);
        Assert.Single(form.Transactions);
        Assert.Equal("removed-account", form.AccountChoice);
        form.AccountChoice = id;
        Assert.True(await form.SubmitAsync());
        Assert.Null(form.Error);
    }

    [Fact]
    public async Task FailedRestoreLeavesTheDialogOpenWithTheError()
    {
        using var store = NewStore();
        const string invalid = "{\"version\":1,\"timestamp\":\"2026-10-03T00:00:00Z\",\"data\":{\"accounts\":[{\"id\":\"duplicate\",\"name\":\"A\"},{\"id\":\"duplicate\",\"name\":\"B\"}]}}";
        var form = new RestoreBackupViewModel(store, invalid, "invalid.dmx");
        await form.PreparationTask;
        Assert.False(await form.SubmitAsync());
        Assert.NotNull(form.Error);
        Assert.False(form.Working);
    }

    [Fact]
    public async Task PerformAsyncWaitsUntilTheUiReceivesItsResult()
    {
        using var store = NewStore();
        var ui = new System.Collections.Concurrent.ConcurrentQueue<Action>();
        store.Dispatch = ui.Enqueue;
        var applied = false;
        var work = store.PerformAsync(_ => true, _ => applied = true);
        var deadline = DateTime.UtcNow.AddSeconds(5);
        while (ui.IsEmpty && DateTime.UtcNow < deadline) await Task.Delay(10);
        Assert.False(work.IsCompleted);
        Assert.False(applied);
        Assert.True(ui.TryDequeue(out var action));
        action();
        await work.WaitAsync(TimeSpan.FromSeconds(5));
        Assert.True(applied);
    }

    [Fact]
    public void TransactionFormMergesUntouchedFieldsAndRejectsOverlappingEdits()
    {
        using var store = NewStore();
        var account = AddAccount(store);
        AddTransaction(store, account, 12.125, "Original");
        store.Reload();
        var id = Assert.Single(new JournalViewModel(store).Rows).Transaction.Id;
        var initial = store.Engine.TransactionDraft(id, [], Today);
        var form = new TransactionFormViewModel(store, initial) { Description = "Local" };
        store.Engine.SaveTransaction(initial with { Amount = 25.125 });
        var remoteAmount = store.Engine.TransactionDraft(id, [], Today).Amount;
        Assert.True(form.Submit());
        var merged = store.Engine.TransactionDraft(id, [], Today);
        Assert.Equal(remoteAmount, merged.Amount);
        Assert.Equal("Local", merged.Description);
        var conflict = new TransactionFormViewModel(store, merged) { Description = "Draft preserved" };
        store.Engine.SaveTransaction(merged with { Description = "Remote" });
        Assert.False(conflict.Submit());
        Assert.NotNull(conflict.Error);
        Assert.Equal("Draft preserved", conflict.Description);
        Assert.Equal("Remote", store.Engine.TransactionDraft(id, [], Today).Description);
    }

    [Fact]
    public void CategoryFiltersFollowCreationRenameAndRemoval()
    {
        using var store = NewStore();
        using var journal = new JournalViewModel(store);
        using var budget = new BudgetViewModel(store);
        using var scheduled = new ScheduledViewModel(store);
        var draft = store.Engine.CategoryDraft(null) with { Name = "Fixture catégorie" };
        var id = store.Engine.SaveCategory(draft);
        store.Reload();
        Assert.Contains(journal.CategoryChoices, item => item.Id == id);
        Assert.Contains(budget.CategoryChoices, item => item.Id == id);
        Assert.Contains(scheduled.CategoryChoices, item => item.Id == id);
        journal.Categories = [id]; budget.Categories = [id]; scheduled.Categories = [id];
        store.Engine.SaveCategory(store.Engine.CategoryDraft(id) with { Name = "Renamed" });
        store.Reload();
        Assert.Equal("Renamed", journal.CategoryChoices.Single(item => item.Id == id).Name);
        store.Engine.DeleteCategory(id); store.Reload();
        Assert.Empty(journal.Categories); Assert.Empty(budget.Categories); Assert.Empty(scheduled.Categories);
    }

    [Fact]
    public void ShellActivatesOnlyTheVisiblePageAndOffersWhatsNew()
    {
        using var store = NewStore();
        AddAccount(store);
        store.Run(engine => engine.ApplySettingsChange(new SettingsChange.SetLastSeenVersion("1.0.22")));

        using var shell = new ShellViewModel(store);
        Assert.Same(shell.Dashboard, shell.CurrentPage);
        Assert.True(shell.Dashboard.IsActive);
        Assert.False(shell.Journal.IsActive);

        shell.NavigateCommand.Execute(AppRoute.Transactions);
        Assert.Same(shell.Journal, shell.CurrentPage);
        Assert.True(shell.Journal.IsActive);
        Assert.False(shell.Dashboard.IsActive);
        Assert.Equal("Journal", shell.RouteTitle);
        Assert.True(shell.ShowsBalances);

        shell.PresentWhatsNewIfNeeded();
        var request = Assert.IsType<FormRequest.WhatsNew>(store.Form);
        Assert.NotNull(request);

        var whatsNew = Assert.IsType<WhatsNewViewModel>(FormFactory.Create(store, store.Form!));
        Assert.True(whatsNew.Submit());
        Assert.Equal(AppInfo.Version, store.Settings.LastSeenVersion);
    }

    [Fact]
    public void SettingsViewModelDrivesThemeAndAccent()
    {
        using var store = NewStore();
        var settings = new SettingsViewModel(store);

        settings.Theme = Theme.Dark;
        Assert.Equal(Theme.Dark, store.Settings.Theme);

        var accent = settings.AccentColors[0];
        settings.SetAccentCommand.Execute(accent);
        Assert.Equal(accent, store.Settings.AccentColor);

        settings.UseDefaultAccentCommand.Execute(null);
        Assert.Null(store.Settings.AccentColor);
        Assert.True(settings.IsDefaultAccent);
    }
}
