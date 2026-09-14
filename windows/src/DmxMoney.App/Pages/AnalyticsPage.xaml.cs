using System.ComponentModel;
using DmxMoney.Interop;
using DmxMoney.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;

namespace DmxMoney.App;

/// <summary>Choix d'une plage dans la liste déroulante.</summary>
public sealed record TimeRangeOption(TimeRange Value, string Label);

public sealed partial class AnalyticsPage : Page
{
    public AnalyticsViewModel ViewModel { get; private set; } = null!;

    private IReadOnlyList<TimeRangeOption> rangeOptions = [];
    private bool syncing;

    public AnalyticsPage()
    {
        InitializeComponent();
        NavigationCacheMode = NavigationCacheMode.Required;
    }

    protected override void OnNavigatedTo(NavigationEventArgs args)
    {
        var shell = (ShellViewModel)args.Parameter;
        if (!ReferenceEquals(ViewModel, shell.Analytics))
        {
            if (ViewModel is not null)
            {
                ViewModel.PropertyChanged -= OnViewModelChanged;
            }
            ViewModel = shell.Analytics;
            ViewModel.PropertyChanged += OnViewModelChanged;
            rangeOptions = [.. ViewModel.Ranges.Select(range => new TimeRangeOption(range, Format.TimeRangeLabel(range)))];
            RangeBox.ItemsSource = rangeOptions;
        }
        // Bindings n'existe que si la page a des x:Bind hors des modèles de données : nul ici.
        Bindings?.Update();
        UpdateVisuals();
    }

    private void OnViewModelChanged(object? sender, PropertyChangedEventArgs args) => UpdateVisuals();

    private void UpdateVisuals()
    {
        if (ViewModel.View is not { } view || ViewModel.Query is null)
        {
            return;
        }
        syncing = true;
        RangeBox.SelectedItem = rangeOptions.FirstOrDefault(option => option.Value == ViewModel.Range);
        CustomDates.Visibility = ViewModel.IsCustomRange ? Visibility.Visible : Visibility.Collapsed;
        MonthStartsOnFirst.Visibility = ViewModel.IsCustomRange ? Visibility.Collapsed : Visibility.Visible;
        MonthStartsOnFirst.IsChecked = ViewModel.MonthStartsOnFirst;
        StartPicker.Date = ViewModel.CustomStart;
        EndPicker.Date = ViewModel.CustomEnd;
        syncing = false;

        var history = view.BalanceHistory;
        BalanceChart.TooltipProvider = index =>
        (
            history.Series.Select(series => new TooltipEntry(
                series.Name,
                series.Color,
                index < series.Values.Length ? Money.Format(series.Values[index]) : string.Empty)),
            null
        );
        BalanceChart.Update(
            [.. history.Series.Select(series => new ChartLineData(series.Id, series.Name, series.Color, series.Values))],
            history.Labels,
            history.FullLabels);
        BalanceLegend.ItemsSource = history.Series;

        UpdateDonut(ExpenseDonut, ExpenseEmpty, ExpenseLegend, view.ExpensesByCategory);
        UpdateDonut(IncomeDonut, IncomeEmpty, IncomeLegend, view.IncomeByCategory);

        var bars = view.IncomeVsExpenses;
        ComparisonChart.TooltipProvider = index =>
        (
            [
                new TooltipEntry("Revenus", "#10b981", Money.Format(bars[index].Income)),
                new TooltipEntry("Dépenses", "#ef4444", Money.Format(bars[index].Expenses)),
            ],
            null
        );
        ComparisonChart.Update(
            [.. bars.Select(bar => new ChartBarGroup(bar.Label, [bar.Income, bar.Expenses]))],
            ["#10b981", "#ef4444"]);
    }

    private static void UpdateDonut(DonutChartView donut, TextBlock empty, ItemsControl legend, IReadOnlyList<CategorySlice> slices)
    {
        var visible = slices.Where(slice => !slice.Hidden && slice.Value > 0).ToList();
        donut.Visibility = visible.Count > 0 ? Visibility.Visible : Visibility.Collapsed;
        empty.Visibility = visible.Count > 0 ? Visibility.Collapsed : Visibility.Visible;
        donut.Update(
            [.. slices.Select(slice => new DonutSliceData(slice.Category.Id, slice.Category.Name, slice.Value, slice.Category.Color, slice.Hidden))],
            "Total",
            Money.Rounded(visible.Sum(slice => slice.Value)));
        legend.ItemsSource = slices;
    }

    private void OnRangeChanged(object sender, SelectionChangedEventArgs args)
    {
        if (!syncing && RangeBox.SelectedItem is TimeRangeOption option)
        {
            ViewModel.Range = option.Value;
        }
    }

    private void OnMonthStartChanged(object sender, RoutedEventArgs args)
    {
        if (!syncing)
        {
            ViewModel.MonthStartsOnFirst = MonthStartsOnFirst.IsChecked == true;
        }
    }

    private void OnStartChanged(CalendarDatePicker sender, CalendarDatePickerDateChangedEventArgs args)
    {
        if (!syncing && args.NewDate is { } date)
        {
            ViewModel.CustomStart = date;
        }
    }

    private void OnEndChanged(CalendarDatePicker sender, CalendarDatePickerDateChangedEventArgs args)
    {
        if (!syncing && args.NewDate is { } date)
        {
            ViewModel.CustomEnd = date;
        }
    }

    private void OnToggleExpense(object sender, RoutedEventArgs args)
    {
        if (sender is FrameworkElement { Tag: CategorySlice slice })
        {
            ViewModel.ToggleExpenseCategoryCommand.Execute(slice);
        }
    }

    private void OnToggleIncome(object sender, RoutedEventArgs args)
    {
        if (sender is FrameworkElement { Tag: CategorySlice slice })
        {
            ViewModel.ToggleIncomeCategoryCommand.Execute(slice);
        }
    }
}
