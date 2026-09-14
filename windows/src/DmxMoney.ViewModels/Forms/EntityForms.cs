using CommunityToolkit.Mvvm.ComponentModel;
using DmxMoney.Interop;

namespace DmxMoney.ViewModels;

public sealed partial class AccountFormViewModel : FormViewModel
{
    private readonly string? id;

    [ObservableProperty]
    private string name;

    [ObservableProperty]
    private string accountType;

    [ObservableProperty]
    private string amountText;

    [ObservableProperty]
    private string? group;

    [ObservableProperty]
    private string color;

    [ObservableProperty]
    private string icon;

    public AccountFormViewModel(EngineStore store, AccountDraft draft) : base(store)
    {
        id = draft.Id;
        name = draft.Name;
        accountType = draft.AccountType;
        amountText = AmountInput.Text(draft.InitialBalance);
        group = draft.Group;
        color = draft.Color;
        icon = draft.Icon;
    }

    public bool IsEditing => id is not null;

    public override string Title => IsEditing ? "Modifier le compte" : "Nouveau compte";

    public override string SubmitTitle => IsEditing ? "Mettre à jour" : "Créer";

    public IReadOnlyList<string> AccountTypes { get; } = DmxFfiMethods.AccountTypes();

    public IReadOnlyList<string> Groups => Store.Settings.CustomGroups;

    public IReadOnlyList<string> Colors { get; } = DmxFfiMethods.CategoryColors();

    /// <summary>Changer de type reprend l'icône et la couleur par défaut du noyau.</summary>
    partial void OnAccountTypeChanged(string value)
    {
        var defaults = DmxFfiMethods.AccountTypeDefaults(value);
        Icon = defaults.Icon;
        Color = defaults.Color;
    }

    public override bool Submit()
    {
        var balance = string.IsNullOrWhiteSpace(AmountText) ? 0 : AmountInput.Parse(AmountText);
        if (balance is null)
        {
            Error = "Saisissez un montant valide";
            return false;
        }
        var draft = new AccountDraft(id, Name, AccountType, balance.Value, Color, Icon, Group);
        Error = Store.Attempt(engine => engine.SaveAccount(draft));
        if (Error is not null)
        {
            return false;
        }
        Store.ShowToast(IsEditing ? "Compte mis à jour" : "Compte créé");
        return true;
    }
}

public sealed partial class CategoryFormViewModel : FormViewModel
{
    private readonly string? id;

    [ObservableProperty]
    private string name;

    [ObservableProperty]
    private string icon;

    [ObservableProperty]
    private string color;

    public CategoryFormViewModel(EngineStore store, CategoryDraft draft) : base(store)
    {
        id = draft.Id;
        name = draft.Name;
        icon = draft.Icon;
        color = draft.Color;
    }

    public bool IsEditing => id is not null;

    public override string Title => IsEditing ? "Modifier la catégorie" : "Nouvelle catégorie";

    public override string SubmitTitle => IsEditing ? "Modifier" : "Ajouter";

    public IReadOnlyList<string> Icons { get; } = DmxFfiMethods.IconPickerNames();

    public IReadOnlyList<string> Colors { get; } = DmxFfiMethods.CategoryColors();

    public override bool Submit()
    {
        var draft = new CategoryDraft(id, Name, Icon, Color);
        Error = Store.Attempt(engine => engine.SaveCategory(draft));
        if (Error is not null)
        {
            return false;
        }
        Store.ShowToast(IsEditing ? "Catégorie modifiée" : "Catégorie ajoutée");
        return true;
    }
}

public sealed partial class TransactionFormViewModel : FormViewModel
{
    private readonly string? id;

    [ObservableProperty]
    private TransactionType kind;

    [ObservableProperty]
    private string description;

    [ObservableProperty]
    private string amountText;

    [ObservableProperty]
    private DateTimeOffset date;

    [ObservableProperty]
    private string accountId;

    [ObservableProperty]
    private string? toAccountId;

    [ObservableProperty]
    private string categoryId;

    public TransactionFormViewModel(EngineStore store, TransactionDraft draft) : base(store)
    {
        id = draft.Id;
        kind = draft.Kind;
        description = draft.Description;
        amountText = AmountInput.Text(draft.Amount);
        date = DayString.ToDate(draft.Date);
        accountId = draft.AccountId;
        toAccountId = draft.ToAccountId;
        categoryId = draft.CategoryId;
    }

    public bool IsEditing => id is not null;

    public override string Title => IsEditing ? "Modifier la transaction" : "Nouvelle transaction";

    public bool IsTransfer => Kind == TransactionType.Transfer;

    public string AccountLabel => IsTransfer ? "Compte source" : "Compte";

    public IReadOnlyList<Account> Accounts => Store.Accounts;

    public IReadOnlyList<Category> Categories => Store.SelectableCategories;

    public IReadOnlyList<KindOption> Kinds { get; } =
    [
        new(TransactionType.Expense, "Dépense"),
        new(TransactionType.Income, "Revenu"),
        new(TransactionType.Transfer, "Virement"),
    ];

    public KindOption SelectedKind
    {
        get => Kinds.First(option => option.Value == Kind);
        set => Kind = value.Value;
    }

    /// <summary>Date pour le sélecteur natif (nullable côté WinUI).</summary>
    public DateTimeOffset? DatePicked
    {
        get => Date;
        set => Date = value ?? Date;
    }

    partial void OnKindChanged(TransactionType value)
    {
        OnPropertyChanged(nameof(IsTransfer));
        OnPropertyChanged(nameof(AccountLabel));
        OnPropertyChanged(nameof(SelectedKind));
    }

    partial void OnDateChanged(DateTimeOffset value) => OnPropertyChanged(nameof(DatePicked));

    public override bool Submit()
    {
        var amount = AmountInput.Parse(AmountText);
        if (amount is null or <= 0)
        {
            Error = "Saisissez un montant valide";
            return false;
        }
        var draft = new TransactionDraft(id, Kind, DayString.FromDate(Date), amount.Value, Description, CategoryId, AccountId, ToAccountId);
        Error = Store.Attempt(engine => engine.SaveTransaction(draft));
        if (Error is not null)
        {
            return false;
        }
        Store.ShowToast(IsEditing
            ? "Transaction mise à jour"
            : IsTransfer ? "Virement ajouté" : "Transaction ajoutée");
        return true;
    }
}

public sealed partial class BudgetFormViewModel : FormViewModel
{
    private readonly string? id;

    [ObservableProperty]
    private string name;

    [ObservableProperty]
    private string amountText;

    [ObservableProperty]
    private string categoryId;

    [ObservableProperty]
    private string? accountId;

    public BudgetFormViewModel(EngineStore store, BudgetDraft draft) : base(store)
    {
        id = draft.Id;
        name = draft.Name;
        amountText = AmountInput.Text(draft.Amount);
        categoryId = draft.CategoryId;
        accountId = draft.AccountId;
    }

    public bool IsEditing => id is not null;

    public override string Title => IsEditing ? "Modifier le budget" : "Nouveau budget";

    public override string SubmitTitle => IsEditing ? "Modifier" : "Créer";

    public IReadOnlyList<Account> Accounts => Store.Accounts;

    public IReadOnlyList<Category> Categories => Store.SelectableCategories;

    public override bool Submit()
    {
        var amount = AmountInput.Parse(AmountText);
        if (amount is null or <= 0)
        {
            Error = "Saisissez un montant valide";
            return false;
        }
        var draft = new BudgetDraft(id, Name, amount.Value, CategoryId, AccountId);
        Error = Store.Attempt(engine => engine.SaveBudget(draft));
        if (Error is not null)
        {
            return false;
        }
        Store.ShowToast(IsEditing ? "Budget modifié" : "Budget ajouté");
        return true;
    }
}

public sealed partial class ScheduledFormViewModel : FormViewModel
{
    private readonly string? id;

    [ObservableProperty]
    private TransactionType kind;

    [ObservableProperty]
    private string description;

    [ObservableProperty]
    private string amountText;

    [ObservableProperty]
    private Periodicity frequency;

    [ObservableProperty]
    private DateTimeOffset nextDate;

    [ObservableProperty]
    private bool hasEndDate;

    [ObservableProperty]
    private DateTimeOffset endDate;

    [ObservableProperty]
    private string accountId;

    [ObservableProperty]
    private string? toAccountId;

    [ObservableProperty]
    private string categoryId;

    [ObservableProperty]
    private string? budgetId;

    public ScheduledFormViewModel(EngineStore store, ScheduledDraft draft) : base(store)
    {
        id = draft.Id;
        kind = draft.Kind;
        description = draft.Description;
        amountText = AmountInput.Text(draft.Amount);
        frequency = draft.Frequency;
        nextDate = DayString.ToDate(draft.NextDate);
        hasEndDate = draft.EndDate is not null;
        endDate = DayString.ToDate(draft.EndDate ?? draft.NextDate);
        accountId = draft.AccountId;
        toAccountId = draft.ToAccountId;
        categoryId = draft.CategoryId;
        budgetId = draft.BudgetId;
        Budgets = store.Read(engine => engine.BudgetsList()) ?? [];
    }

    public bool IsEditing => id is not null;

    public override string Title => IsEditing ? "Modifier la transaction" : "Nouvelle transaction récurrente";

    public override string SubmitTitle => IsEditing ? "Modifier" : "Ajouter";

    public IReadOnlyList<Budget> Budgets { get; }

    public IReadOnlyList<PeriodicityOption> Frequencies { get; } =
        [.. DmxFfiMethods.AllPeriodicities().Select(frequency => new PeriodicityOption(frequency, DmxFfiMethods.PeriodicityFormLabel(frequency)))];

    public PeriodicityOption SelectedFrequency
    {
        get => Frequencies.First(option => option.Value == Frequency);
        set => Frequency = value.Value;
    }

    public IReadOnlyList<KindOption> Kinds { get; } =
    [
        new(TransactionType.Expense, "Dépense"),
        new(TransactionType.Income, "Revenu"),
        new(TransactionType.Transfer, "Virement"),
    ];

    public KindOption SelectedKind
    {
        get => Kinds.First(option => option.Value == Kind);
        set => Kind = value.Value;
    }

    /// <summary>Dates pour les sélecteurs natifs (nullables côté WinUI).</summary>
    public DateTimeOffset? NextDatePicked
    {
        get => NextDate;
        set => NextDate = value ?? NextDate;
    }

    public DateTimeOffset? EndDatePicked
    {
        get => EndDate;
        set => EndDate = value ?? EndDate;
    }

    public IReadOnlyList<Account> Accounts => Store.Accounts;

    public IReadOnlyList<Category> Categories => Store.SelectableCategories;

    public bool IsTransfer => Kind == TransactionType.Transfer;

    public bool CanLinkBudget => Kind == TransactionType.Expense;

    /// <summary>Un budget lié impose sa catégorie et, s'il en a un, son compte (règle du noyau).</summary>
    public Budget? LinkedBudget => CanLinkBudget && BudgetId is { } id ? Budgets.FirstOrDefault(budget => budget.Id == id) : null;

    partial void OnKindChanged(TransactionType value)
    {
        OnPropertyChanged(nameof(IsTransfer));
        OnPropertyChanged(nameof(CanLinkBudget));
        OnPropertyChanged(nameof(LinkedBudget));
        OnPropertyChanged(nameof(SelectedKind));
    }

    partial void OnFrequencyChanged(Periodicity value) => OnPropertyChanged(nameof(SelectedFrequency));

    partial void OnNextDateChanged(DateTimeOffset value) => OnPropertyChanged(nameof(NextDatePicked));

    partial void OnEndDateChanged(DateTimeOffset value) => OnPropertyChanged(nameof(EndDatePicked));

    partial void OnBudgetIdChanged(string? value)
    {
        if (Budgets.FirstOrDefault(budget => budget.Id == value) is { } budget)
        {
            CategoryId = budget.Category;
            if (budget.AccountId is { } account)
            {
                AccountId = account;
            }
        }
        OnPropertyChanged(nameof(LinkedBudget));
    }

    public override bool Submit()
    {
        var amount = AmountInput.Parse(AmountText);
        if (amount is null or <= 0)
        {
            Error = "Saisissez un montant valide";
            return false;
        }
        var draft = new ScheduledDraft(
            id,
            Description,
            amount.Value,
            Kind,
            CategoryId,
            AccountId,
            ToAccountId,
            Frequency,
            DayString.FromDate(NextDate),
            HasEndDate ? DayString.FromDate(EndDate) : null,
            Kind == TransactionType.Expense ? BudgetId : null);
        Error = Store.Attempt(engine => engine.SaveScheduled(draft));
        if (Error is not null)
        {
            return false;
        }
        Store.ShowToast(IsEditing ? "Échéance modifiée" : "Échéance ajoutée");
        return true;
    }
}

public sealed partial class FakeTransactionFormViewModel : FormViewModel
{
    private readonly string? id;

    [ObservableProperty]
    private TransactionType kind;

    [ObservableProperty]
    private string description;

    [ObservableProperty]
    private string amountText;

    [ObservableProperty]
    private DateTimeOffset date;

    [ObservableProperty]
    private string accountId;

    [ObservableProperty]
    private string? toAccountId;

    [ObservableProperty]
    private string categoryId;

    public FakeTransactionFormViewModel(EngineStore store, FakeTransactionDraft draft) : base(store)
    {
        id = draft.Id;
        kind = draft.Kind;
        description = draft.Description;
        amountText = AmountInput.Text(draft.Amount);
        date = DayString.ToDate(draft.Date);
        accountId = draft.AccountId;
        toAccountId = draft.ToAccountId;
        categoryId = draft.CategoryId;
    }

    public bool IsEditing => id is not null;

    public override string Title => IsEditing ? "Modifier la transaction fictive" : "Nouvelle transaction fictive";

    public override string SubmitTitle => IsEditing ? "Enregistrer" : "Ajouter";

    public bool IsTransfer => Kind == TransactionType.Transfer;

    public IReadOnlyList<Account> Accounts => Store.Accounts;

    public IReadOnlyList<Category> Categories => Store.SelectableCategories;

    public IReadOnlyList<KindOption> Kinds { get; } =
    [
        new(TransactionType.Expense, "Dépense"),
        new(TransactionType.Income, "Revenu"),
        new(TransactionType.Transfer, "Virement"),
    ];

    public KindOption SelectedKind
    {
        get => Kinds.First(option => option.Value == Kind);
        set => Kind = value.Value;
    }

    /// <summary>Date pour le sélecteur natif (nullable côté WinUI).</summary>
    public DateTimeOffset? DatePicked
    {
        get => Date;
        set => Date = value ?? Date;
    }

    partial void OnKindChanged(TransactionType value)
    {
        OnPropertyChanged(nameof(IsTransfer));
        OnPropertyChanged(nameof(SelectedKind));
    }

    partial void OnDateChanged(DateTimeOffset value) => OnPropertyChanged(nameof(DatePicked));

    public override bool Submit()
    {
        var amount = AmountInput.Parse(AmountText);
        if (amount is null or <= 0)
        {
            Error = "Saisissez un montant valide";
            return false;
        }
        var today = Store.Today;
        var draft = new FakeTransactionDraft(id, DayString.FromDate(Date), Description, amount.Value, Kind, AccountId, ToAccountId, CategoryId);
        Error = Store.Attempt(engine => engine.SaveFakeTransaction(draft, today));
        if (Error is not null)
        {
            return false;
        }
        Store.ShowToast(IsEditing ? "Transaction fictive mise à jour" : "Transaction fictive ajoutée");
        return true;
    }
}
