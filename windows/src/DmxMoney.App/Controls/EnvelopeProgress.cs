using DmxMoney.Interop;
using DmxMoney.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace DmxMoney.App;

/// <summary>Consommation d'une enveloppe : barre colorée et pourcentage.</summary>
public sealed class EnvelopeProgress : ContentControl
{
    public static readonly DependencyProperty EnvelopeProperty = DependencyProperty.Register(
        nameof(Envelope), typeof(object), typeof(EnvelopeProgress), new PropertyMetadata(null, (sender, _) => ((EnvelopeProgress)sender).Apply()));

    private readonly ProgressBarView bar = new() { Height = 6 };
    private readonly TextBlock used = new() { FontSize = 11 };
    private readonly TextBlock over = new() { FontSize = 11, HorizontalAlignment = HorizontalAlignment.Right };

    public object? Envelope
    {
        get => GetValue(EnvelopeProperty);
        set => SetValue(EnvelopeProperty, value);
    }

    public EnvelopeProgress()
    {
        used.Foreground = Palette.Resource("TextFillColorSecondaryBrush");
        over.Foreground = Palette.Expense;
        var footer = new Grid();
        footer.Children.Add(used);
        footer.Children.Add(over);
        var stack = new StackPanel { Spacing = 4 };
        stack.Children.Add(bar);
        stack.Children.Add(footer);
        Content = stack;
        HorizontalContentAlignment = HorizontalAlignment.Stretch;
    }

    private void Apply()
    {
        if (Envelope is not BudgetEnvelope envelope)
        {
            return;
        }
        var isOver = envelope.Remaining < 0;
        bar.Update(envelope.Progress, isOver ? "#ef4444" : "#10b981");
        used.Text = $"{Format.PercentTight(envelope.Progress)} utilisé";
        over.Text = isOver ? $"{Money.Format(Math.Abs(envelope.Remaining))} au-dessus" : string.Empty;
    }
}
