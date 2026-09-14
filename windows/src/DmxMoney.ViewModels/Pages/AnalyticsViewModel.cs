using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using DmxMoney.Interop;

namespace DmxMoney.ViewModels;

public sealed partial class AnalyticsViewModel : PageViewModel
{
    [ObservableProperty]
    private AnalyticsQuery? query;

    [ObservableProperty]
    private AnalyticsView? view;

    public AnalyticsViewModel(EngineStore store) : base(store) => Refresh();

    public IReadOnlyList<TimeRange> Ranges { get; } = DmxFfiMethods.AllTimeRanges();

    public static string RangeLabel(TimeRange range) => DmxFfiMethods.TimeRangeLabel(range);

    public TimeRange Range
    {
        get => Query?.Range ?? TimeRange.Month;
        set
        {
            if (Query is not null && value != Query.Range)
            {
                Store.Apply(new SettingsChange.SetAnalyticsTimeRange(value));
            }
        }
    }

    public bool IsCustomRange => Range == TimeRange.Custom;

    public bool MonthStartsOnFirst
    {
        get => Query?.MonthStartsOnFirst ?? false;
        set => Store.Apply(new SettingsChange.SetAnalyticsMonthStartsOnFirst(value));
    }

    public DateTimeOffset CustomStart
    {
        get => DayString.ToDate(Query?.CustomStart ?? View?.StartDate ?? Store.Today);
        set => Store.Apply(new SettingsChange.SetAnalyticsCustomStartDate(DayString.FromDate(value)));
    }

    public DateTimeOffset CustomEnd
    {
        get => DayString.ToDate(Query?.CustomEnd ?? View?.EndDate ?? Store.Today);
        set => Store.Apply(new SettingsChange.SetAnalyticsCustomEndDate(DayString.FromDate(value)));
    }

    public override void Refresh()
    {
        var accounts = Store.SelectedAccountIds.ToArray();
        var today = Store.Today;
        var query = Store.Read(engine => engine.AnalyticsQuery(accounts));
        if (query is null)
        {
            return;
        }
        Query = query;
        View = Store.Read(engine => engine.Analytics(query, today));
        OnPropertyChanged(nameof(Range));
        OnPropertyChanged(nameof(IsCustomRange));
        OnPropertyChanged(nameof(MonthStartsOnFirst));
        OnPropertyChanged(nameof(CustomStart));
        OnPropertyChanged(nameof(CustomEnd));
    }

    /// <summary>Clic sur la légende : masque ou réaffiche une catégorie (choix mémorisé).</summary>
    [RelayCommand]
    private void ToggleExpenseCategory(CategorySlice slice) => Toggle(slice, TransactionType.Expense);

    [RelayCommand]
    private void ToggleIncomeCategory(CategorySlice slice) => Toggle(slice, TransactionType.Income);

    private void Toggle(CategorySlice slice, TransactionType kind)
        => Store.Apply(new SettingsChange.SetAnalyticsCategoryHidden(kind, slice.Category.Id, !slice.Hidden));
}
