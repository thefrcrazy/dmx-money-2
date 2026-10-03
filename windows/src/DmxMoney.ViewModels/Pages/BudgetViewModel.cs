using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using DmxMoney.Interop;

namespace DmxMoney.ViewModels;

public sealed partial class BudgetViewModel : PageViewModel
{
    [ObservableProperty]
    private BudgetOverview? view;

    [ObservableProperty]
    private string search = string.Empty;

    private bool reconcilingCategories;

    [ObservableProperty]
    private IReadOnlyList<string> categories = [];

    public BudgetViewModel(EngineStore store) : base(store) => Refresh();

    public IReadOnlyList<Category> CategoryChoices => Store.SelectableCategories;

    public override void Refresh()
    {
        var validIds = CategoryChoices.Select(category => category.Id).ToHashSet();
        var filtered = Categories.Where(validIds.Contains).ToArray();
        if (!Categories.SequenceEqual(filtered)) { reconcilingCategories = true; Categories = filtered; reconcilingCategories = false; }
        OnPropertyChanged(nameof(CategoryChoices));
        var query = new BudgetQuery([.. Store.SelectedAccountIds], Search, [.. Categories]);
        var today = Store.Today;
        View = Store.Read(engine => engine.Budget(query, today));
        OnPropertyChanged(nameof(StateLabel));
        OnPropertyChanged(nameof(PaceLabel));
        OnPropertyChanged(nameof(SuggestionCount));
    }

    partial void OnSearchChanged(string value) => Refresh();

    partial void OnCategoriesChanged(IReadOnlyList<string> value) { if (!reconcilingCategories) Refresh(); }

    public string StateLabel => View is null ? string.Empty : DmxFfiMethods.BudgetStateLabel(View.State);

    public string PaceLabel => View is null
        ? string.Empty
        : View.PaceDelta > 0
            ? $"{Money.Rounded(View.PaceDelta)} au-dessus du rythme"
            : $"{Money.Rounded(Math.Abs(View.PaceDelta))} sous le rythme";

    public int SuggestionCount => View?.Suggestions.Length ?? 0;

    [RelayCommand]
    private void NewBudget() => Store.Present(new FormRequest.BudgetForm(null));

    [RelayCommand]
    private void NewBudgetForCategory(string categoryId) => Store.Present(new FormRequest.BudgetForm(null, categoryId));

    [RelayCommand]
    private void Edit(string budgetId) => Store.Present(new FormRequest.BudgetForm(budgetId));

    [RelayCommand]
    private void ShowSuggestions() => Store.Present(new FormRequest.BudgetSuggestions());

    [RelayCommand]
    private void Delete(string budgetId) => Store.Confirm(
        "Supprimer le budget",
        "Ce budget sera supprimé et les échéances liées seront simplement déliées.",
        () => Store.Run(engine => engine.DeleteBudget(budgetId), "Budget supprimé"));
}
