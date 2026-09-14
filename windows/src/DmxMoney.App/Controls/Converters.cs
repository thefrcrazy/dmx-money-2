using DmxMoney.Interop;
using DmxMoney.ViewModels;
using Microsoft.UI;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Data;
using Microsoft.UI.Xaml.Media;
using Windows.UI;

namespace DmxMoney.App;

public static class Palette
{
    /// <summary>Convertit « #RRGGBB » (tel que stocké en base) en couleur.</summary>
    public static Color ToColor(string? hex, Color fallback)
    {
        if (string.IsNullOrWhiteSpace(hex))
        {
            return fallback;
        }
        var value = hex.TrimStart('#');
        if (value.Length == 3)
        {
            value = string.Concat(value.Select(character => new string(character, 2)));
        }
        if (value.Length is not (6 or 8) || !uint.TryParse(value, System.Globalization.NumberStyles.HexNumber, null, out var number))
        {
            return fallback;
        }
        var hasAlpha = value.Length == 8;
        var alpha = hasAlpha ? (byte)((number >> 24) & 0xFF) : (byte)255;
        var red = (byte)((number >> 16) & 0xFF);
        var green = (byte)((number >> 8) & 0xFF);
        var blue = (byte)(number & 0xFF);
        return Color.FromArgb(alpha, red, green, blue);
    }

    public static SolidColorBrush ToBrush(string? hex, Color fallback) => new(ToColor(hex, fallback));

    public static Brush Resource(string key) => (Brush)Application.Current.Resources[key];

    public static Brush Income => Resource("DmxIncome");

    public static Brush Expense => Resource("DmxExpense");

    public static Brush Transfer => Resource("DmxTransfer");

    public static Brush ForKind(TransactionType kind) => kind switch
    {
        TransactionType.Income => Income,
        TransactionType.Transfer => Transfer,
        _ => Expense,
    };
}

/// <summary>« #RRGGBB » → pinceau.</summary>
public sealed class ColorHexConverter : IValueConverter
{
    public object Convert(object value, Type targetType, object parameter, string language)
        => Palette.ToBrush(value as string, Colors.Gray);

    public object ConvertBack(object value, Type targetType, object parameter, string language)
        => throw new NotSupportedException();
}

/// <summary>Montant → pinceau vert (positif) ou rouge (négatif).</summary>
public sealed class AmountBrushConverter : IValueConverter
{
    public object Convert(object value, Type targetType, object parameter, string language)
        => value is double amount && amount < 0 ? Palette.Expense : Palette.Resource("TextFillColorPrimaryBrush");

    public object ConvertBack(object value, Type targetType, object parameter, string language)
        => throw new NotSupportedException();
}

/// <summary>Montant → texte fr-FR fourni par le noyau.</summary>
public sealed class MoneyConverter : IValueConverter
{
    public object Convert(object value, Type targetType, object parameter, string language)
        => value is double amount ? Money.Format(amount) : string.Empty;

    public object ConvertBack(object value, Type targetType, object parameter, string language)
        => throw new NotSupportedException();
}

/// <summary>Date « YYYY-MM-DD » → « 14 sept. ».</summary>
public sealed class ShortDateConverter : IValueConverter
{
    public object Convert(object value, Type targetType, object parameter, string language)
        => value is string date && date.Length >= 10 ? DayFormat.Short(date) : string.Empty;

    public object ConvertBack(object value, Type targetType, object parameter, string language)
        => throw new NotSupportedException();
}

public sealed class BoolToVisibilityConverter : IValueConverter
{
    public object Convert(object value, Type targetType, object parameter, string language)
    {
        var flag = value is bool boolean && boolean;
        if (parameter is string text && text.Equals("invert", StringComparison.OrdinalIgnoreCase))
        {
            flag = !flag;
        }
        return flag ? Visibility.Visible : Visibility.Collapsed;
    }

    public object ConvertBack(object value, Type targetType, object parameter, string language)
        => value is Visibility.Visible;
}

public sealed class NullToVisibilityConverter : IValueConverter
{
    public object Convert(object value, Type targetType, object parameter, string language)
    {
        var empty = value is null || (value is string text && text.Length == 0);
        if (parameter is string option && option.Equals("invert", StringComparison.OrdinalIgnoreCase))
        {
            empty = !empty;
        }
        return empty ? Visibility.Collapsed : Visibility.Visible;
    }

    public object ConvertBack(object value, Type targetType, object parameter, string language)
        => throw new NotSupportedException();
}
