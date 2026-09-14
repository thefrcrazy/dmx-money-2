using System.ComponentModel;
using DmxMoney.Interop;
using DmxMoney.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Navigation;
using Windows.System;

namespace DmxMoney.App;

public sealed partial class PredictionsPage : Page
{
    public PredictionsViewModel ViewModel { get; private set; } = null!;

    private IReadOnlyList<TimeRangeOption> rangeOptions = [];
    private bool syncing;

    public PredictionsPage()
    {
        InitializeComponent();
        NavigationCacheMode = NavigationCacheMode.Required;
    }

    protected override void OnNavigatedTo(NavigationEventArgs args)
    {
        var shell = (ShellViewModel)args.Parameter;
        if (!ReferenceEquals(ViewModel, shell.Predictions))
        {
            if (ViewModel is not null)
            {
                ViewModel.PropertyChanged -= OnViewModelChanged;
            }
            ViewModel = shell.Predictions;
            ViewModel.PropertyChanged += OnViewModelChanged;
            rangeOptions = [.. ViewModel.Ranges.Select(range => new TimeRangeOption(range, Format.TimeRangeLabel(range)))];
            RangeBox.ItemsSource = rangeOptions;
            MarkerLegend.ItemsSource = PredictionsViewModel.MarkerLegend
                .Select(item => new MarkerLegendItem(item.Color, item.Label))
                .ToList();
        }
        Bindings.Update();
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
        EndPicker.Visibility = ViewModel.IsCustomRange ? Visibility.Visible : Visibility.Collapsed;
        EndPicker.Date = ViewModel.CustomEnd;
        MonthStartsOnFirst.Visibility = ViewModel.IsCustomRange ? Visibility.Collapsed : Visibility.Visible;
        MonthStartsOnFirst.IsChecked = ViewModel.MonthStartsOnFirst;
        IntradayLow.IsChecked = ViewModel.ShowIntradayLow;
        syncing = false;

        // Transactions fictives
        var fake = view.FakeTransactions;
        FakeList.ItemsSource = fake;
        FakeEmpty.Visibility = fake.Length == 0 ? Visibility.Visible : Visibility.Collapsed;
        FakeSummary.Visibility = fake.Length == 0 ? Visibility.Collapsed : Visibility.Visible;
        ClearFakeButton.Visibility = fake.Length == 0 ? Visibility.Collapsed : Visibility.Visible;
        FakeCount.Text = $"{view.EnabledFakeCount}/{fake.Length} {(fake.Length > 1 ? "simulations" : "simulation")} {(view.EnabledFakeCount == 1 ? "active" : "actives")}";
        FakeImpact.Text = $"Impact période : {(view.FakeImpact >= 0 ? "+" : string.Empty)}{Money.Format(view.FakeImpact)}";
        FakeImpact.Foreground = Format.PositiveBrush(view.FakeImpact);

        // Projection
        ProjectionTitle.Text = $"Projection sur {view.TitleLabel} (Journalière)";
        var lines = view.Accounts
            .Select(series => new ChartLineData(series.Id, series.Name, series.Color, series.Closes))
            .ToList();
        if (ViewModel.ShowIntradayLow)
        {
            lines.AddRange(view.Accounts.Select(series => new ChartLineData(
                series.Id + "-low",
                $"{series.Name} — point bas",
                series.Color,
                series.Lows,
                Dashed: true,
                Filled: false,
                InLegend: false)));
        }
        var references = new List<ChartReference> { new(0, "#ef4444") };
        if (view.AlertThreshold > 0)
        {
            references.Add(new ChartReference(view.AlertThreshold, "#f97316", Dashed: true));
        }
        ProjectionChart.Stepped = true;
        ProjectionChart.TooltipProvider = index =>
        {
            var marker = view.Markers.FirstOrDefault(candidate => candidate.Index == index);
            var notes = new List<string>();
            if (marker is not null)
            {
                if (marker.CrossingNames.Length > 0)
                {
                    var label = marker.Severity == Severity.Danger ? "Solde négatif" : "Sous le seuil d’alerte";
                    notes.Add($"{label} : {string.Join(", ", marker.CrossingNames)}");
                }
                if (marker.IntradayNames.Length > 0)
                {
                    var label = marker.IntradaySeverity == Severity.Danger ? "Point bas négatif" : "Point bas sous le seuil";
                    notes.Add($"{label} : {string.Join(", ", marker.IntradayNames)}");
                }
            }
            return (
                view.Accounts.Select(series => new TooltipEntry(
                    series.Name,
                    series.Color,
                    index < series.Closes.Length ? Money.Format(series.Closes[index]) : string.Empty)),
                notes.Count == 0 ? null : string.Join("\n", notes));
        };
        ProjectionChart.Update(
            lines,
            view.Labels,
            view.FullLabels,
            references,
            [.. view.Markers.Select(marker => new ChartMarkerData((int)marker.Index, marker.StrokeColor))]);

        // Jours à surveiller
        var risks = ViewModel.RiskDays;
        RiskCard.Visibility = view.IntradayRiskCount > 0 ? Visibility.Visible : Visibility.Collapsed;
        RiskTitle.Text = $"Jours à surveiller ({view.IntradayRiskCount})";
        RiskDays.ItemsSource = risks.Take(8).ToList();
        var others = Math.Max(risks.Count - 8, 0);
        RiskMore.Visibility = others > 0 ? Visibility.Visible : Visibility.Collapsed;
        RiskMore.Text = others > 0 ? $"+{others} {(others > 1 ? "autres jours" : "autre jour")} sur la période." : string.Empty;

        // Totaux
        CurrentTotal.Text = Money.Format(view.CurrentTotalBalance);
        MidpointTotal.Text = Money.Format(view.MidpointBalance);
        MidpointTotal.Foreground = view.MidpointBalance >= view.CurrentTotalBalance ? Palette.Income : Palette.Expense;
        FinalLabel.Text = $"PROJECTION AU {view.EndLabel.ToUpperInvariant()}";
        FinalTotal.Text = Money.Format(view.FinalBalance);
        FinalTotal.Foreground = view.FinalBalance >= view.CurrentTotalBalance ? Palette.Income : Palette.Expense;
    }

    private void OnRangeChanged(object sender, SelectionChangedEventArgs args)
    {
        if (!syncing && RangeBox.SelectedItem is TimeRangeOption option)
        {
            ViewModel.Range = option.Value;
        }
    }

    private void OnEndChanged(CalendarDatePicker sender, CalendarDatePickerDateChangedEventArgs args)
    {
        if (!syncing && args.NewDate is { } date)
        {
            ViewModel.CustomEnd = date;
        }
    }

    private void OnMonthStartChanged(object sender, RoutedEventArgs args)
    {
        if (!syncing)
        {
            ViewModel.MonthStartsOnFirst = MonthStartsOnFirst.IsChecked == true;
        }
    }

    private void OnIntradayChanged(object sender, RoutedEventArgs args)
    {
        if (!syncing)
        {
            ViewModel.ShowIntradayLow = IntradayLow.IsChecked == true;
            UpdateVisuals();
        }
    }

    private void OnToggleFake(object sender, RoutedEventArgs args)
    {
        if (!syncing && sender is FrameworkElement { Tag: string id })
        {
            ViewModel.ToggleFakeTransactionCommand.Execute(id);
        }
    }

    private void OnEditFake(object sender, RoutedEventArgs args)
    {
        if (sender is FrameworkElement { Tag: string id })
        {
            ViewModel.EditFakeTransactionCommand.Execute(id);
        }
    }

    private void OnRemoveFake(object sender, RoutedEventArgs args)
    {
        if (sender is FrameworkElement { Tag: string id })
        {
            ViewModel.RemoveFakeTransactionCommand.Execute(id);
        }
    }

    private void OnThresholdCommitted(object sender, RoutedEventArgs args) => ViewModel.CommitThreshold();

    private void OnThresholdKeyDown(object sender, KeyRoutedEventArgs args)
    {
        if (args.Key == VirtualKey.Enter)
        {
            args.Handled = true;
            ViewModel.CommitThreshold();
        }
    }
}

/// <summary>Ligne de légende des marqueurs de la projection.</summary>
public sealed class MarkerLegendItem : StackPanel
{
    public MarkerLegendItem(string colorHex, string label)
    {
        Orientation = Orientation.Horizontal;
        Spacing = 6;
        Children.Add(new Microsoft.UI.Xaml.Shapes.Rectangle
        {
            Width = 16,
            Height = 2,
            RadiusX = 1,
            RadiusY = 1,
            Fill = Format.Brush(colorHex),
            VerticalAlignment = VerticalAlignment.Center,
        });
        Children.Add(new TextBlock
        {
            Text = label,
            FontSize = 11,
            Foreground = Palette.Resource("TextFillColorSecondaryBrush"),
        });
    }
}
