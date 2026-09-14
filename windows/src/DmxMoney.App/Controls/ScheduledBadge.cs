using DmxMoney.Interop;
using DmxMoney.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace DmxMoney.App;

/// <summary>Pastille d'une échéance : « Terminé », « Budget », ou date de fin.</summary>
public sealed class ScheduledBadge : ContentControl
{
    public static readonly DependencyProperty RowProperty = DependencyProperty.Register(
        nameof(Row), typeof(object), typeof(ScheduledBadge), new PropertyMetadata(null, (sender, _) => ((ScheduledBadge)sender).Apply()));

    private readonly TextBlock end = new() { FontSize = 11 };
    private readonly TextBlock badge = new() { FontSize = 10, FontWeight = Microsoft.UI.Text.FontWeights.Bold };
    private readonly Border badgeHost;

    public object? Row
    {
        get => GetValue(RowProperty);
        set => SetValue(RowProperty, value);
    }

    public ScheduledBadge()
    {
        end.Foreground = Palette.Resource("TextFillColorSecondaryBrush");
        badgeHost = new Border
        {
            Child = badge,
            CornerRadius = new CornerRadius(4),
            Padding = new Thickness(6, 1, 6, 1),
            HorizontalAlignment = HorizontalAlignment.Left,
            Visibility = Visibility.Collapsed,
        };
        var stack = new StackPanel { Spacing = 2 };
        stack.Children.Add(end);
        stack.Children.Add(badgeHost);
        Content = stack;
    }

    private void Apply()
    {
        if (Row is not ScheduledRow row)
        {
            return;
        }
        end.Text = row.Scheduled.EndDate is { } endDate ? $"→ {DayFormat.Medium(endDate)}" : string.Empty;
        end.Visibility = row.Scheduled.EndDate is null ? Visibility.Collapsed : Visibility.Visible;

        if (row.IsEnded)
        {
            badge.Text = "Terminé";
            badge.Foreground = Palette.Expense;
            badgeHost.Background = Format.Tint("#ef4444");
            badgeHost.Visibility = Visibility.Visible;
        }
        else if (row.Scheduled.BudgetId is not null)
        {
            badge.Text = "Budget";
            badge.Foreground = Palette.Transfer;
            badgeHost.Background = Format.Tint("#6366f1");
            badgeHost.Visibility = Visibility.Visible;
        }
        else
        {
            badgeHost.Visibility = Visibility.Collapsed;
        }
    }
}
