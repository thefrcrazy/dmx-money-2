using DmxMoney.Interop;
using DmxMoney.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace DmxMoney.App;

/// <summary>
/// Pastille « budget restant » d'une ligne du journal : masquée quand la ligne n'est liée à aucun budget.
/// Un contrôle évite les liaisons XAML sur un chemin nullable.
/// </summary>
public sealed class BudgetChip : ContentControl
{
    public static readonly DependencyProperty RowProperty = DependencyProperty.Register(
        nameof(Row), typeof(object), typeof(BudgetChip), new PropertyMetadata(null, (sender, _) => ((BudgetChip)sender).Apply()));

    private readonly TextBlock label = new()
    {
        FontSize = 10,
        FontWeight = Microsoft.UI.Text.FontWeights.Bold,
    };

    private readonly Border chip;

    public object? Row
    {
        get => GetValue(RowProperty);
        set => SetValue(RowProperty, value);
    }

    public BudgetChip()
    {
        label.Foreground = Palette.Transfer;
        chip = new Border
        {
            Child = label,
            CornerRadius = new CornerRadius(4),
            Padding = new Thickness(6, 2, 6, 2),
            BorderThickness = new Thickness(1),
            BorderBrush = Palette.Transfer,
            HorizontalAlignment = HorizontalAlignment.Right,
            Visibility = Visibility.Collapsed,
        };
        Content = chip;
        HorizontalContentAlignment = HorizontalAlignment.Right;
    }

    private void Apply()
    {
        if (Row is JournalRow { Budget: { } budget })
        {
            label.Text = Money.Format(budget.Remaining);
            chip.Visibility = Visibility.Visible;
            ToolTipService.SetToolTip(chip, budget.BudgetName);
        }
        else
        {
            chip.Visibility = Visibility.Collapsed;
        }
    }
}
