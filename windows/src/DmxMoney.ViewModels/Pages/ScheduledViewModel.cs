using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using DmxMoney.Interop;

namespace DmxMoney.ViewModels;

public sealed partial class ScheduledViewModel : PageViewModel
{
    [ObservableProperty]
    private ScheduledView? view;

    [ObservableProperty]
    private string search = string.Empty;

    [ObservableProperty]
    private IReadOnlyList<string> categories = [];

    [ObservableProperty]
    private IReadOnlyList<Periodicity> frequencies = [];

    public ScheduledViewModel(EngineStore store) : base(store) => Refresh();

    public IReadOnlyList<Periodicity> AllFrequencies { get; } = DmxFfiMethods.AllPeriodicities();

    public IReadOnlyList<ScheduledDueRange> DueRanges { get; } = DmxFfiMethods.AllDueRanges();

    public static string FrequencyLabel(Periodicity frequency) => DmxFfiMethods.PeriodicityLabel(frequency);

    public static string DueRangeLabel(ScheduledDueRange range) => DmxFfiMethods.DueRangeLabel(range);

    public ScheduledDueRange DueRange
    {
        get => Store.Settings.ScheduledDueRange;
        set
        {
            if (value != Store.Settings.ScheduledDueRange)
            {
                Store.Apply(new SettingsChange.SetScheduledDueRange(value));
                OnPropertyChanged();
            }
        }
    }

    public override void Refresh()
    {
        var query = new ScheduledQuery(
            [.. Store.SelectedAccountIds],
            Store.Settings.ScheduledDueRange,
            Search,
            [.. Categories],
            [.. Frequencies]);
        var today = Store.Today;
        View = Store.Read(engine => engine.Scheduled(query, today));
        OnPropertyChanged(nameof(DueRange));
        OnPropertyChanged(nameof(SuggestionCount));
    }

    partial void OnSearchChanged(string value) => Refresh();

    partial void OnCategoriesChanged(IReadOnlyList<string> value) => Refresh();

    partial void OnFrequenciesChanged(IReadOnlyList<Periodicity> value) => Refresh();

    public int SuggestionCount => View?.Suggestions.Length ?? 0;

    [RelayCommand]
    private void NewScheduled() => Store.Present(new FormRequest.ScheduledForm(null));

    [RelayCommand]
    private void Edit(string id) => Store.Present(new FormRequest.ScheduledForm(id));

    [RelayCommand]
    private void ShowSuggestions() => Store.Present(new FormRequest.ScheduledSuggestions());

    [RelayCommand]
    private void Delete(string id) => Store.Confirm(
        "Supprimer la transaction récurrente",
        "Êtes-vous sûr de vouloir supprimer cette transaction récurrente ?",
        () => Store.Run(engine => engine.DeleteScheduled(id), "Transaction récurrente supprimée"));
}
