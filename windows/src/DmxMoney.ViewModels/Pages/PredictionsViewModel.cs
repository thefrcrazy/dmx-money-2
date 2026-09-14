using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using DmxMoney.Interop;

namespace DmxMoney.ViewModels;

public sealed partial class PredictionsViewModel : PageViewModel
{
    /// <summary>Légende des marqueurs, identique sur toutes les plateformes.</summary>
    public static readonly (string Color, string Label)[] MarkerLegend =
    [
        ("#ef4444", "Solde négatif en fin de journée"),
        ("#f97316", "Passage sous le seuil d’alerte"),
        ("#a855f7", "Point bas négatif, rattrapé par un revenu"),
        ("#eab308", "Point bas sous le seuil, rattrapé par un revenu"),
    ];

    [ObservableProperty]
    private PredictionQuery? query;

    [ObservableProperty]
    private PredictionView? view;

    /// <summary>Affichage du point bas journalier (préférence locale, comme en 1.x).</summary>
    [ObservableProperty]
    private bool showIntradayLow;

    [ObservableProperty]
    private string thresholdText = string.Empty;

    public PredictionsViewModel(EngineStore store) : base(store) => Refresh();

    public IReadOnlyList<TimeRange> Ranges { get; } = DmxFfiMethods.AllTimeRanges();

    public static string RangeLabel(TimeRange range) => DmxFfiMethods.TimeRangeLabel(range);

    public TimeRange Range
    {
        get => Query?.Range ?? TimeRange.Month;
        set
        {
            if (Query is not null && value != Query.Range)
            {
                Store.Apply(new SettingsChange.SetPredictionTimeRange(value));
            }
        }
    }

    public bool IsCustomRange => Range == TimeRange.Custom;

    public bool MonthStartsOnFirst
    {
        get => Query?.MonthStartsOnFirst ?? false;
        set => Store.Apply(new SettingsChange.SetPredictionMonthStartsOnFirst(value));
    }

    public DateTimeOffset CustomEnd
    {
        get => DayString.ToDate(Query?.CustomEndDate ?? View?.EndDate ?? Store.Today);
        set => Store.Apply(new SettingsChange.SetPredictionCustomEndDate(DayString.FromDate(value)));
    }

    public IReadOnlyList<FakeTransactionRow> FakeTransactions => View?.FakeTransactions ?? [];

    public IReadOnlyList<PredictionMarker> RiskDays => View is null
        ? []
        : [.. View.Markers.Where(marker => marker.IntradaySeverity is not null)];

    public override void Refresh()
    {
        var accounts = Store.SelectedAccountIds.ToArray();
        var today = Store.Today;
        var query = Store.Read(engine => engine.PredictionQuery(accounts));
        if (query is null)
        {
            return;
        }
        Query = query;
        View = Store.Read(engine => engine.Predictions(query, today));
        ThresholdText = AmountInput.Text(query.AlertThreshold);
        OnPropertyChanged(nameof(Range));
        OnPropertyChanged(nameof(IsCustomRange));
        OnPropertyChanged(nameof(MonthStartsOnFirst));
        OnPropertyChanged(nameof(CustomEnd));
        OnPropertyChanged(nameof(FakeTransactions));
        OnPropertyChanged(nameof(RiskDays));
    }

    public void CommitThreshold()
    {
        var current = Query?.AlertThreshold ?? 0;
        var value = string.IsNullOrWhiteSpace(ThresholdText) ? 0 : AmountInput.Parse(ThresholdText) ?? current;
        if (Math.Abs(value - current) > 0.0001)
        {
            Store.Apply(new SettingsChange.SetPredictionAlertThreshold(value));
        }
    }

    [RelayCommand]
    private void NewFakeTransaction() => Store.Present(new FormRequest.FakeTransactionForm(null));

    [RelayCommand]
    private void EditFakeTransaction(string id) => Store.Present(new FormRequest.FakeTransactionForm(id));

    [RelayCommand]
    private void ToggleFakeTransaction(string id) => Store.Run(engine => engine.ToggleFakeTransaction(id));

    [RelayCommand]
    private void RemoveFakeTransaction(string id)
        => Store.Run(engine => engine.DeleteFakeTransactions([id]), "Transaction fictive retirée");

    [RelayCommand]
    private void ClearFakeTransactions()
    {
        var accounts = Store.SelectedAccountIds.ToArray();
        Store.Run(engine => engine.ClearAppliedFakeTransactions(accounts), "Transactions fictives retirées");
    }
}
