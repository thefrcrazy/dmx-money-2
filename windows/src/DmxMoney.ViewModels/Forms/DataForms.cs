using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using DmxMoney.Interop;

namespace DmxMoney.ViewModels;

/// <summary>Gestion des groupes de comptes : ajout, renommage, ordre, suppression.</summary>
public sealed partial class AccountGroupsViewModel : FormViewModel
{
    [ObservableProperty]
    private string newGroup = string.Empty;

    public AccountGroupsViewModel(EngineStore store) : base(store)
    {
    }

    public override string Title => "Gérer les groupes";

    public override bool ShowsSubmit => false;

    public IReadOnlyList<string> Groups => [.. Store.Settings.EffectiveGroupOrder.Where(group => group != "Non groupés")];

    public override bool Submit() => true;

    [RelayCommand]
    private void Add()
    {
        var name = NewGroup.Trim();
        if (name.Length == 0)
        {
            return;
        }
        Store.Run(engine => engine.ApplySettingsChange(new SettingsChange.AddCustomGroup(name)));
        NewGroup = string.Empty;
        OnPropertyChanged(nameof(Groups));
    }

    [RelayCommand]
    private void Rename((string OldName, string NewName) rename)
    {
        var name = rename.NewName.Trim();
        if (name.Length == 0 || name == rename.OldName)
        {
            return;
        }
        Store.Run(engine => engine.ApplySettingsChange(new SettingsChange.RenameCustomGroup(rename.OldName, name)));
        OnPropertyChanged(nameof(Groups));
    }

    [RelayCommand]
    private void Move((string Group, int Offset) move)
    {
        var order = Groups.ToList();
        var index = order.IndexOf(move.Group);
        var target = index + move.Offset;
        if (index < 0 || target < 0 || target >= order.Count)
        {
            return;
        }
        (order[index], order[target]) = (order[target], order[index]);
        Store.Run(engine => engine.ApplySettingsChange(new SettingsChange.SetCustomGroupsOrder([.. order])));
        OnPropertyChanged(nameof(Groups));
    }

    [RelayCommand]
    private void Delete(string group) => Store.Confirm(
        "Supprimer le groupe",
        $"Les comptes du groupe « {group} » seront déplacés dans « Non groupés ».",
        () =>
        {
            Store.Run(engine => engine.ApplySettingsChange(new SettingsChange.DeleteCustomGroup(group)));
            OnPropertyChanged(nameof(Groups));
        });
}

/// <summary>Suggestions de budgets déduites du Journal (6 derniers mois).</summary>
public sealed partial class BudgetSuggestionsViewModel : FormViewModel
{
    public BudgetSuggestionsViewModel(EngineStore store) : base(store) => Reload();

    public override string Title => "Suggestions du journal";

    public override bool ShowsSubmit => false;

    public IReadOnlyList<BudgetSuggestion> Suggestions { get; private set; } = [];

    public override bool Submit() => true;

    private void Reload()
    {
        var query = new BudgetQuery([.. Store.SelectedAccountIds], string.Empty, []);
        var today = Store.Today;
        Suggestions = Store.Read(engine => engine.Budget(query, today))?.Suggestions ?? [];
        OnPropertyChanged(nameof(Suggestions));
    }

    [RelayCommand]
    private void Accept(BudgetSuggestion suggestion)
    {
        var accounts = Store.SelectedAccountIds.ToArray();
        var today = Store.Today;
        Store.Run(engine => engine.AcceptBudgetSuggestion(suggestion.Key, accounts, today), "Budget ajouté");
        Reload();
    }

    [RelayCommand]
    private void Dismiss(BudgetSuggestion suggestion)
    {
        Store.Run(engine => engine.DismissBudgetSuggestion(suggestion.Key), "Suggestion supprimée");
        Reload();
    }
}

/// <summary>Suggestions d'échéances repérées dans le Journal (au moins deux mois consécutifs).</summary>
public sealed partial class ScheduledSuggestionsViewModel : FormViewModel
{
    public ScheduledSuggestionsViewModel(EngineStore store) : base(store) => Reload();

    public override string Title => "Suggestions du journal";

    public override bool ShowsSubmit => false;

    public IReadOnlyList<ScheduledSuggestion> Suggestions { get; private set; } = [];

    public override bool Submit() => true;

    private void Reload()
    {
        var query = new ScheduledQuery([.. Store.SelectedAccountIds], ScheduledDueRange.All, string.Empty, [], []);
        var today = Store.Today;
        Suggestions = Store.Read(engine => engine.Scheduled(query, today))?.Suggestions ?? [];
        OnPropertyChanged(nameof(Suggestions));
    }

    [RelayCommand]
    private void Accept(ScheduledSuggestion suggestion)
    {
        var accounts = Store.SelectedAccountIds.ToArray();
        var today = Store.Today;
        Store.Run(engine => engine.AcceptScheduledSuggestion(suggestion.Key, accounts, today), "Suggestion ajoutée à l'échéancier");
        Reload();
    }

    [RelayCommand]
    private void Dismiss(ScheduledSuggestion suggestion)
    {
        Store.Run(engine => engine.DismissScheduledSuggestion(suggestion.Key), "Suggestion supprimée");
        Reload();
    }
}

/// <summary>Restauration d'une sauvegarde .dmx : remplacer ou fusionner.</summary>
public sealed partial class RestoreBackupViewModel : FormViewModel
{
    private readonly string content;

    [ObservableProperty]
    private RestoreMode mode = RestoreMode.Replace;

    [ObservableProperty]
    private bool working;

    public RestoreBackupViewModel(EngineStore store, string content, string fileName) : base(store)
    {
        this.content = content;
        FileName = fileName;
        try
        {
            Summary = store.Engine.InspectBackup(content);
        }
        catch (Exception error)
        {
            Error = $"Le fichier de sauvegarde est corrompu. ({EngineStore.Message(error)})";
        }
    }

    public string FileName { get; }

    public BackupSummary? Summary { get; }

    /// <summary>Restauration en cours (attendue par les tests).</summary>
    public Task? RestoreTask { get; private set; }

    public override string Title => "Importer une sauvegarde";

    public override string SubmitTitle => Mode == RestoreMode.Replace ? "Remplacer mes données" : "Fusionner";

    public string ModeDetail => Mode == RestoreMode.Replace
        ? "Toutes les données actuelles seront remplacées par celles de la sauvegarde."
        : "Les éléments de la sauvegarde sont ajoutés ; les éléments déjà présents sont conservés.";

    public string? BackupDate => Summary is null ? null : DayFormat.Long(Summary.Timestamp[..10]);

    partial void OnModeChanged(RestoreMode value)
    {
        OnPropertyChanged(nameof(SubmitTitle));
        OnPropertyChanged(nameof(ModeDetail));
    }

    public override bool Submit()
    {
        if (Summary is null || Working)
        {
            return false;
        }
        Working = true;
        var payload = content;
        var mode = Mode;
        RestoreTask = Store.PerformAsync(
            engine => engine.RestoreBackup(payload, mode),
            _ => Store.ShowToast("Import réussi"),
            message =>
            {
                Error = message;
                Working = false;
            });
        return true;
    }
}

/// <summary>Assistant d'import de relevé : colonnes, compte, catégories, confirmation.</summary>
public sealed partial class StatementImportViewModel : FormViewModel
{
    public enum Step
    {
        Columns,
        Account,
        Categories,
        Confirm,
    }

    public const string NewAccountId = "__new__";
    public const string NewCategoryId = "__new__";

    private readonly string content;

    [ObservableProperty]
    private Step currentStep = Step.Columns;

    [ObservableProperty]
    private string separator = ";";

    [ObservableProperty]
    private bool hasHeader = true;

    [ObservableProperty]
    private CsvPreview? preview;

    [ObservableProperty]
    private string? accountChoice;

    [ObservableProperty]
    private string newAccountName = string.Empty;

    [ObservableProperty]
    private string newAccountType = "Courant";

    [ObservableProperty]
    private string finalBalanceText = string.Empty;

    [ObservableProperty]
    private bool importing;

    public StatementImportViewModel(EngineStore store, string content, string fileName) : base(store)
    {
        this.content = content;
        FileName = fileName;
        Format = DmxFfiMethods.StatementFormatForFile(fileName) ?? StatementFormat.Csv;
        Mapping = new CsvColumnMapping(0, 1, 3, null);
        if (Format == StatementFormat.Csv)
        {
            separator = DmxFfiMethods.DetectCsvSeparator(content);
            RefreshPreview();
        }
        else
        {
            Parse();
            currentStep = Step.Account;
        }
    }

    public string FileName { get; }

    /// <summary>Import en cours (attendu par les tests).</summary>
    public Task? ImportTask { get; private set; }

    public StatementFormat Format { get; }

    public CsvColumnMapping Mapping { get; private set; }

    public IReadOnlyList<ParsedStatementTransaction> Transactions { get; private set; } = [];

    public IReadOnlyList<string> Sources { get; private set; } = [];

    public Dictionary<string, string> CategoryMapping { get; } = [];

    public IReadOnlyList<string> AccountTypes { get; } = DmxFfiMethods.AccountTypes();

    public IReadOnlyList<Account> Accounts => Store.Accounts;

    public IReadOnlyList<Category> Categories => Store.SelectableCategories;

    public override string Title => Format switch
    {
        StatementFormat.Csv => "Assistant d'import CSV",
        StatementFormat.Qif => "Import QIF",
        _ => "Import OFX",
    };

    public override string SubmitTitle => CurrentStep == Step.Confirm ? "Importer maintenant" : "Suivant";

    public bool IsCsv => Format == StatementFormat.Csv;

    public bool IsColumnsStep => CurrentStep == Step.Columns;

    public bool IsAccountStep => CurrentStep == Step.Account;

    public bool IsCategoriesStep => CurrentStep == Step.Categories;

    public bool IsConfirmStep => CurrentStep == Step.Confirm;

    public string ConfirmSummary => $"{Transactions.Count} transactions seront importées dans le compte « {TargetAccountName} ».";

    public Step[] VisibleSteps => IsCsv
        ? [Step.Columns, Step.Account, Step.Categories, Step.Confirm]
        : [Step.Account, Step.Categories, Step.Confirm];

    public static string StepLabel(Step step) => step switch
    {
        Step.Columns => "Colonnes",
        Step.Account => "Compte",
        Step.Categories => "Catégories",
        _ => "Confirmation",
    };

    public string TargetAccountName => AccountChoice == NewAccountId
        ? NewAccountName
        : Store.AccountById(AccountChoice)?.Name ?? string.Empty;

    public bool MissingCategoryColumn => IsCsv && Mapping.Category is null;

    public double? ComputedInitialBalance
    {
        get
        {
            if (AccountChoice != NewAccountId || AmountInput.Parse(FinalBalanceText) is not { } final)
            {
                return null;
            }
            return DmxFfiMethods.InitialBalanceFromFinal([.. Transactions], final);
        }
    }

    public bool CanContinue => CurrentStep switch
    {
        Step.Columns => Mapping.Date is not null && Mapping.Amount is not null,
        Step.Account => AccountChoice is not null
            && (AccountChoice != NewAccountId || !string.IsNullOrWhiteSpace(NewAccountName)),
        _ => true,
    };

    /// <summary>Rôle d'une colonne CSV : Date, Montant, Description, Catégorie ou ignorée.</summary>
    public string RoleOf(int column)
    {
        var index = (uint)column;
        if (Mapping.Date == index)
        {
            return "date";
        }
        if (Mapping.Amount == index)
        {
            return "amount";
        }
        if (Mapping.Description == index)
        {
            return "description";
        }
        return Mapping.Category == index ? "category" : "ignore";
    }

    public void SetRole(int column, string role)
    {
        var index = (uint)column;
        var mapping = Mapping;
        mapping = mapping with
        {
            Date = mapping.Date == index ? null : mapping.Date,
            Amount = mapping.Amount == index ? null : mapping.Amount,
            Description = mapping.Description == index ? null : mapping.Description,
            Category = mapping.Category == index ? null : mapping.Category,
        };
        Mapping = role switch
        {
            "date" => mapping with { Date = index },
            "amount" => mapping with { Amount = index },
            "description" => mapping with { Description = index },
            "category" => mapping with { Category = index },
            _ => mapping,
        };
        OnPropertyChanged(nameof(Mapping));
        OnPropertyChanged(nameof(MissingCategoryColumn));
        OnPropertyChanged(nameof(CanContinue));
    }

    partial void OnCurrentStepChanged(Step value)
    {
        OnPropertyChanged(nameof(IsColumnsStep));
        OnPropertyChanged(nameof(IsAccountStep));
        OnPropertyChanged(nameof(IsCategoriesStep));
        OnPropertyChanged(nameof(IsConfirmStep));
        OnPropertyChanged(nameof(SubmitTitle));
        OnPropertyChanged(nameof(ConfirmSummary));
        OnPropertyChanged(nameof(ComputedInitialBalance));
    }

    partial void OnSeparatorChanged(string value) => RefreshPreview();

    partial void OnHasHeaderChanged(bool value) => RefreshPreview();

    partial void OnAccountChoiceChanged(string? value) => OnPropertyChanged(nameof(CanContinue));

    partial void OnNewAccountNameChanged(string value) => OnPropertyChanged(nameof(CanContinue));

    private void RefreshPreview()
    {
        try
        {
            Preview = Store.Engine.PreviewCsv(content, new CsvOptions(Separator, HasHeader));
            Error = null;
        }
        catch (Exception error)
        {
            Error = EngineStore.Message(error);
        }
    }

    private bool Parse()
    {
        try
        {
            var options = IsCsv ? new CsvOptions(Separator, HasHeader) : null;
            var mapping = IsCsv ? Mapping : null;
            Transactions = Store.Engine.ParseStatement(Format, content, options, mapping, Store.Today);
            if (Transactions.Count == 0)
            {
                Error = "Aucune transaction n'a été trouvée dans le fichier.";
                return false;
            }
            Sources = DmxFfiMethods.SourceCategories([.. Transactions]);
            CategoryMapping.Clear();
            foreach (var match in Store.Engine.SuggestCategoryMapping([.. Sources]))
            {
                CategoryMapping[match.Source] = match.CategoryId ?? NewCategoryId;
            }
            Error = null;
            OnPropertyChanged(nameof(Transactions));
            OnPropertyChanged(nameof(Sources));
            return true;
        }
        catch (Exception error)
        {
            Error = EngineStore.Message(error);
            return false;
        }
    }

    /// <summary>Bouton principal : passe à l'étape suivante, ou lance l'import.</summary>
    public override bool Submit()
    {
        if (CurrentStep == Step.Confirm)
        {
            return StartImport();
        }
        Next();
        return false;
    }

    [RelayCommand]
    private void Next()
    {
        switch (CurrentStep)
        {
            case Step.Columns:
                if (Parse())
                {
                    CurrentStep = Step.Account;
                }
                break;
            case Step.Account:
                if (AccountChoice == NewAccountId
                    && !string.IsNullOrWhiteSpace(FinalBalanceText)
                    && AmountInput.Parse(FinalBalanceText) is null)
                {
                    Error = "Saisissez un montant valide";
                    return;
                }
                Error = null;
                CurrentStep = Sources.Count == 0 ? Step.Confirm : Step.Categories;
                break;
            case Step.Categories:
                CurrentStep = Step.Confirm;
                break;
        }
    }

    [RelayCommand]
    private void Back()
    {
        Error = null;
        CurrentStep = CurrentStep switch
        {
            Step.Account => IsCsv ? Step.Columns : Step.Account,
            Step.Categories => Step.Account,
            Step.Confirm => Sources.Count == 0 ? Step.Account : Step.Categories,
            _ => Step.Columns,
        };
    }

    private bool StartImport()
    {
        if (AccountChoice is null || Importing)
        {
            return false;
        }
        ImportTarget target = AccountChoice == NewAccountId
            ? new ImportTarget.NewAccount(NewAccountName.Trim(), NewAccountType, AmountInput.Parse(FinalBalanceText))
            : new ImportTarget.ExistingAccount(AccountChoice);
        var matches = Sources
            .Select(source =>
            {
                var mapped = CategoryMapping.TryGetValue(source, out var value) ? value : NewCategoryId;
                return new CategoryMatch(source, mapped == NewCategoryId ? null : mapped);
            })
            .ToArray();
        var request = new StatementImportRequest([.. Transactions], target, matches);
        Importing = true;
        ImportTask = Store.PerformAsync(
            engine => engine.ImportStatement(request),
            result =>
            {
                var duplicates = result.Duplicates > 0 ? $" ({result.Duplicates} doublons ignorés)" : string.Empty;
                Store.ShowToast($"{result.Imported} transactions importées{duplicates}");
            },
            message =>
            {
                Error = message;
                Importing = false;
            });
        return true;
    }
}

/// <summary>Nouveautés de la version, affichées après une mise à jour.</summary>
public sealed class WhatsNewViewModel : FormViewModel
{
    public WhatsNewViewModel(EngineStore store) : base(store)
    {
    }

    public override string Title => "Nouveautés";

    public override string SubmitTitle => "Continuer";

    public string Version => AppInfo.Version;

    public IReadOnlyList<string> Notes => AppInfo.ReleaseNotes;

    public override bool Submit()
    {
        Store.Apply(new SettingsChange.SetLastSeenVersion(AppInfo.Version));
        return true;
    }
}
