using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using DmxMoney.Interop;

namespace DmxMoney.ViewModels;

public sealed partial class JournalViewModel : PageViewModel
{
    public static readonly (string Id, string Label)[] TypeOptions =
    [
        ("expense", "Dépenses"),
        ("income", "Revenus"),
        ("transfer", "Virements"),
    ];

    public static readonly (string Id, string Label)[] StatusOptions =
    [
        ("checked", "Pointées"),
        ("unchecked", "Non pointées"),
    ];

    public static readonly (string Id, string Label)[] BudgetOptions =
    [
        ("budgeted", "Avec budget"),
        ("unbudgeted", "Hors budget"),
    ];

    private readonly HashSet<string> selection = [];

    [ObservableProperty]
    private JournalView? view;

    [ObservableProperty]
    private string search = string.Empty;

    [ObservableProperty]
    private IReadOnlyList<string> categories = [];

    [ObservableProperty]
    private IReadOnlyList<string> types = [];

    [ObservableProperty]
    private IReadOnlyList<string> statuses = [];

    [ObservableProperty]
    private IReadOnlyList<string> budgetStatuses = [];

    public JournalViewModel(EngineStore store) : base(store) => Refresh();

    public IReadOnlyList<JournalRow> Rows => View?.Rows ?? [];

    public bool HasFilters => View?.HasFilters ?? false;

    public IReadOnlyCollection<string> Selection => selection;

    public int SelectionCount => selection.Count;

    public override void Refresh()
    {
        var query = new JournalQuery(
            [.. Store.SelectedAccountIds],
            Search,
            [.. Categories],
            [.. Types.Select(TransactionTypeFor).Where(type => type is not null).Select(type => type!.Value)],
            [.. Statuses.Select(status => status == "checked" ? CheckStatus.Checked : CheckStatus.Unchecked)],
            [.. BudgetStatuses.Select(status => status == "budgeted" ? BudgetStatus.Budgeted : BudgetStatus.Unbudgeted)]);
        View = Store.Read(engine => engine.Journal(query));
        var visible = Rows.Select(row => row.Transaction.Id).ToHashSet();
        if (selection.RemoveWhere(id => !visible.Contains(id)) > 0)
        {
            NotifySelection();
        }
        OnPropertyChanged(nameof(Rows));
        OnPropertyChanged(nameof(HasFilters));
    }

    private static TransactionType? TransactionTypeFor(string key) => key switch
    {
        "expense" => TransactionType.Expense,
        "income" => TransactionType.Income,
        "transfer" => TransactionType.Transfer,
        _ => null,
    };

    partial void OnSearchChanged(string value) => Refresh();

    partial void OnCategoriesChanged(IReadOnlyList<string> value) => Refresh();

    partial void OnTypesChanged(IReadOnlyList<string> value) => Refresh();

    partial void OnStatusesChanged(IReadOnlyList<string> value) => Refresh();

    partial void OnBudgetStatusesChanged(IReadOnlyList<string> value) => Refresh();

    public void SetSelection(IEnumerable<string> ids)
    {
        selection.Clear();
        foreach (var id in ids)
        {
            selection.Add(id);
        }
        NotifySelection();
    }

    private void NotifySelection()
    {
        OnPropertyChanged(nameof(Selection));
        OnPropertyChanged(nameof(SelectionCount));
    }

    public void ClearFilters()
    {
        Search = string.Empty;
        Categories = [];
        Types = [];
        Statuses = [];
        BudgetStatuses = [];
    }

    [RelayCommand]
    private void NewTransaction() => Store.Present(new FormRequest.TransactionForm(null));

    [RelayCommand]
    private void Edit(string id) => Store.Present(new FormRequest.TransactionForm(id));

    [RelayCommand]
    private void ToggleChecked(string id) => Store.Run(engine => engine.ToggleTransactionsChecked([id]));

    /// <summary>Pointer/Dépointer la sélection : règle du noyau (tout pointé → tout dépointé).</summary>
    [RelayCommand]
    private void ToggleCheckedSelection()
    {
        if (selection.Count == 0)
        {
            return;
        }
        var ids = selection.ToArray();
        Store.Run(engine => engine.ToggleTransactionsChecked(ids));
    }

    [RelayCommand]
    private void Delete(string id) => Store.Confirm(
        "Supprimer",
        "Voulez-vous vraiment supprimer cette transaction ?",
        () => Store.Run(engine => engine.DeleteTransactions([id]), "Transaction supprimée"));

    [RelayCommand]
    private void DeleteSelection()
    {
        if (selection.Count == 0)
        {
            return;
        }
        var ids = selection.ToArray();
        Store.Confirm(
            "Supprimer la sélection",
            $"Voulez-vous vraiment supprimer {Plural.Of(ids.Length, "transaction")} ?",
            () =>
            {
                Store.Run(engine => engine.DeleteTransactions(ids), "Transactions supprimées");
                SetSelection([]);
            },
            confirmTitle: "Tout supprimer");
    }

    public void UpdateDescription(string id, string description)
        => Store.Run(engine => engine.UpdateTransactionDescription(id, description), "Transaction mise à jour");

    /// <summary>Renvoie <c>false</c> si le montant saisi est invalide.</summary>
    public bool UpdateAmount(string id, string text)
    {
        var amount = AmountInput.Parse(text);
        if (amount is null or <= 0)
        {
            Store.ErrorMessage = "Saisissez un montant valide";
            return false;
        }
        Store.Run(engine => engine.UpdateTransactionAmount(id, amount.Value), "Transaction mise à jour");
        return true;
    }
}
