using DmxMoney.Interop;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace DmxMoney.App;

/// <summary>Contenu d'une sauvegarde : comptes, transactions, catégories, échéances, budgets.</summary>
public sealed class BackupSummaryView : ContentControl
{
    public static readonly DependencyProperty SummaryProperty = DependencyProperty.Register(
        nameof(Summary), typeof(object), typeof(BackupSummaryView), new PropertyMetadata(null, (sender, _) => ((BackupSummaryView)sender).Build()));

    private readonly Grid grid = new() { ColumnSpacing = 8 };

    public object? Summary
    {
        get => GetValue(SummaryProperty);
        set => SetValue(SummaryProperty, value);
    }

    public BackupSummaryView()
    {
        Content = grid;
        HorizontalContentAlignment = HorizontalAlignment.Stretch;
    }

    private void Build()
    {
        grid.Children.Clear();
        grid.ColumnDefinitions.Clear();
        if (Summary is not BackupSummary summary)
        {
            return;
        }
        (string Label, uint Value)[] cells =
        [
            ("comptes", summary.Accounts),
            ("transactions", summary.Transactions),
            ("catégories", summary.Categories),
            ("échéances", summary.Scheduled),
            ("budgets", summary.Budgets),
        ];
        for (var index = 0; index < cells.Length; index++)
        {
            grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            var stack = new StackPanel { Spacing = 2 };
            stack.Children.Add(new TextBlock
            {
                Text = cells[index].Value.ToString(),
                FontSize = 16,
                FontWeight = Microsoft.UI.Text.FontWeights.Bold,
                HorizontalAlignment = HorizontalAlignment.Center,
            });
            stack.Children.Add(new TextBlock
            {
                Text = cells[index].Label,
                FontSize = 10,
                Foreground = Palette.Resource("TextFillColorSecondaryBrush"),
                HorizontalAlignment = HorizontalAlignment.Center,
            });
            var border = new Border
            {
                Child = stack,
                CornerRadius = new CornerRadius(8),
                Padding = new Thickness(8),
                Background = (Microsoft.UI.Xaml.Media.Brush)Application.Current.Resources["DmxSubtleBackground"],
            };
            Grid.SetColumn(border, index);
            grid.Children.Add(border);
        }
    }
}
