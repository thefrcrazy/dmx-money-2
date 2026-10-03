using DmxMoney.Interop;
using Microsoft.UI.Xaml.Controls;

namespace DmxMoney.App;

internal static class CategoryFilter
{
    public static void Update(ListView list, IReadOnlyList<Category> choices, IReadOnlyList<string> selectedIds)
    {
        if (list.ItemsSource is not IReadOnlyList<Category> current || !current.SequenceEqual(choices)) list.ItemsSource = choices;
        var selected = selectedIds.ToHashSet();
        if (selected.SetEquals(list.SelectedItems.OfType<Category>().Select(category => category.Id))) return;
        list.SelectedItems.Clear();
        foreach (var category in choices) if (selected.Contains(category.Id)) list.SelectedItems.Add(category);
    }
}
