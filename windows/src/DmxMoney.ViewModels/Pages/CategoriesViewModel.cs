using System.Globalization;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using DmxMoney.Interop;

namespace DmxMoney.ViewModels;

public sealed partial class CategoriesViewModel : PageViewModel
{
    [ObservableProperty]
    private string search = string.Empty;

    public CategoriesViewModel(EngineStore store) : base(store) => Refresh();

    public IReadOnlyList<Category> Visible { get; private set; } = [];

    public override void Refresh()
    {
        var query = Search.Trim();
        Visible = query.Length == 0
            ? Store.Categories
            : [.. Store.Categories.Where(category => CultureInfo.InvariantCulture.CompareInfo.IndexOf(
                category.Name, query, CompareOptions.IgnoreCase | CompareOptions.IgnoreNonSpace) >= 0)];
        OnPropertyChanged(nameof(Visible));
    }

    partial void OnSearchChanged(string value) => Refresh();

    public static bool IsLocked(Category category) => category.Id == "transfer";

    [RelayCommand]
    private void NewCategory() => Store.Present(new FormRequest.CategoryForm(null));

    [RelayCommand]
    private void Edit(string id) => Store.Present(new FormRequest.CategoryForm(id));

    [RelayCommand]
    private void Delete(string id)
    {
        var name = Store.CategoryById(id)?.Name ?? "cette catégorie";
        Store.Confirm(
            "Supprimer la catégorie",
            $"Êtes-vous sûr de vouloir supprimer « {name} » ?",
            () => Store.Run(engine => engine.DeleteCategory(id), "Catégorie supprimée"));
    }
}
