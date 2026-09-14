using System.ComponentModel;
using DmxMoney.Interop;
using DmxMoney.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;

namespace DmxMoney.App;

/// <summary>Choix d'une plage d'échéance dans la liste déroulante.</summary>
public sealed record DueRangeOption(ScheduledDueRange Value, string Label);

/// <summary>Choix d'une fréquence dans le filtre.</summary>
public sealed record FrequencyOption(Periodicity Value, string Label);

public sealed partial class ScheduledPage : Page
{
    public ScheduledViewModel ViewModel { get; private set; } = null!;

    private bool syncing;
    private IReadOnlyList<DueRangeOption> rangeOptions = [];
    private IReadOnlyList<FrequencyOption> frequencyOptions = [];

    public ScheduledPage()
    {
        InitializeComponent();
        NavigationCacheMode = NavigationCacheMode.Required;
    }

    protected override void OnNavigatedTo(NavigationEventArgs args)
    {
        var shell = (ShellViewModel)args.Parameter;
        if (!ReferenceEquals(ViewModel, shell.Scheduled))
        {
            if (ViewModel is not null)
            {
                ViewModel.PropertyChanged -= OnViewModelChanged;
            }
            ViewModel = shell.Scheduled;
            ViewModel.PropertyChanged += OnViewModelChanged;
            syncing = true;
            rangeOptions = [.. ViewModel.DueRanges.Select(range => new DueRangeOption(range, Format.DueRangeLabel(range)))];
            frequencyOptions = [.. ViewModel.AllFrequencies.Select(frequency => new FrequencyOption(frequency, Format.FrequencyLabel(frequency)))];
            RangeBox.ItemsSource = rangeOptions;
            RangeBox.SelectedItem = rangeOptions.FirstOrDefault(option => option.Value == ViewModel.DueRange);
            CategoriesList.ItemsSource = shell.Store.Categories;
            FrequenciesList.ItemsSource = frequencyOptions;
            syncing = false;
        }
        Bindings.Update();
        UpdateVisuals();
    }

    private void OnViewModelChanged(object? sender, PropertyChangedEventArgs args) => UpdateVisuals();

    private void UpdateVisuals()
    {
        syncing = true;
        var selected = rangeOptions.FirstOrDefault(option => option.Value == ViewModel.DueRange);
        if (!ReferenceEquals(RangeBox.SelectedItem, selected))
        {
            RangeBox.SelectedItem = selected;
        }
        syncing = false;

        var view = ViewModel.View;
        Rows.ItemsSource = view?.Rows;
        var isEmpty = (view?.Rows.Length ?? 0) == 0;
        EmptyState.Visibility = isEmpty ? Visibility.Visible : Visibility.Collapsed;
        EmptyTitle.Text = view?.HasFilters == true
            ? "Aucune transaction récurrente ne correspond aux filtres."
            : "Aucune transaction récurrente configurée";
        EmptyMessage.Text = view?.HasFilters == true ? string.Empty : "Ajoute une transaction récurrente pour commencer.";

        SuggestionsButton.Visibility = ViewModel.SuggestionCount > 0 ? Visibility.Visible : Visibility.Collapsed;
        SuggestionsLabel.Text = $"Suggestions ({ViewModel.SuggestionCount})";
        CategoriesButton.Content = ViewModel.Categories.Count == 0 ? "Toutes les catégories" : $"Catégories ({ViewModel.Categories.Count})";
        FrequenciesButton.Content = ViewModel.Frequencies.Count == 0 ? "Toutes les fréquences" : $"Fréquences ({ViewModel.Frequencies.Count})";
    }

    private void OnRangeChanged(object sender, SelectionChangedEventArgs args)
    {
        if (!syncing && RangeBox.SelectedItem is DueRangeOption option)
        {
            ViewModel.DueRange = option.Value;
        }
    }

    private void OnCategoriesChanged(object sender, SelectionChangedEventArgs args)
    {
        if (!syncing)
        {
            ViewModel.Categories = [.. CategoriesList.SelectedItems.OfType<Category>().Select(category => category.Id)];
        }
    }

    private void OnFrequenciesChanged(object sender, SelectionChangedEventArgs args)
    {
        if (!syncing)
        {
            ViewModel.Frequencies = [.. FrequenciesList.SelectedItems.OfType<FrequencyOption>().Select(option => option.Value)];
        }
    }

    private void OnEdit(object sender, RoutedEventArgs args)
    {
        if (sender is FrameworkElement { Tag: string id })
        {
            ViewModel.EditCommand.Execute(id);
        }
    }

    private void OnDelete(object sender, RoutedEventArgs args)
    {
        if (sender is FrameworkElement { Tag: string id })
        {
            ViewModel.DeleteCommand.Execute(id);
        }
    }
}
