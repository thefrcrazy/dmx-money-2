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

        Assert.Equal(StatementFormat.Csv, wizard.Format);
        Assert.NotNull(wizard.Preview);
        wizard.SetRole(2, "description");
        wizard.SetRole(3, "category");
        Assert.True(wizard.CanContinue);

        wizard.NextCommand.Execute(null);
        Assert.Equal(StatementImportViewModel.Step.Account, wizard.CurrentStep);
        Assert.Equal(2, wizard.Transactions.Count);

        wizard.AccountChoice = StatementImportViewModel.NewAccountId;
        wizard.NewAccountName = "Compte importé";
        wizard.FinalBalanceText = "1 487,50";
        wizard.NextCommand.Execute(null);
        Assert.Equal(StatementImportViewModel.Step.Categories, wizard.CurrentStep);
        Assert.Equal(2, wizard.Sources.Count);

        wizard.NextCommand.Execute(null);
        Assert.Equal(StatementImportViewModel.Step.Confirm, wizard.CurrentStep);
        Assert.Equal(0, wizard.ComputedInitialBalance!.Value, 3);

        Assert.True(wizard.Submit());
        await wizard.ImportTask!;

        var account = Assert.Single(store.Accounts);
        Assert.Equal("Compte importé", account.Name);
        Assert.Equal(1487.5, store.Balances.CurrentBalance, 3);
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
