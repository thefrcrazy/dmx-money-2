using System.Globalization;
using DmxMoney.Interop;

namespace DmxMoney.ViewModels;

/// <summary>Formats fr-FR fournis par le noyau (espaces fines, « € » insécable).</summary>
public static class Money
{
    public static string Format(double amount) => DmxFfiMethods.FormatCurrency(amount);

    /// <summary>Montant arrondi à l'euro, comme les cartes de la vue d'ensemble.</summary>
    public static string Rounded(double amount) => DmxFfiMethods.FormatCurrencyRounded(amount);

    public static string Signed(double amount, TransactionType kind, bool showMinus = true) => kind switch
    {
        TransactionType.Income => "+" + Format(amount),
        TransactionType.Expense => showMinus ? "-" + Format(amount) : Format(amount),
        _ => Format(amount),
    };
}

public static class DayFormat
{
    public static string Numeric(string date) => DmxFfiMethods.FormatDateNumeric(date);

    public static string Short(string date) => DmxFfiMethods.FormatDateShort(date);

    public static string Medium(string date) => DmxFfiMethods.FormatDateMedium(date);

    public static string Long(string date) => DmxFfiMethods.FormatDateLong(date);

    public static string Relative(long days) => days switch
    {
        0 => "Aujourd'hui",
        1 => "Demain",
        -1 => "En retard d'un jour",
        < 0 => $"En retard de {-days} jours",
        _ => $"Dans {days} jours",
    };
}

/// <summary>Conversion entre les dates « YYYY-MM-DD » du noyau et <see cref="DateTimeOffset"/>.</summary>
public static class DayString
{
    public static DateTimeOffset ToDate(string value)
    {
        var text = value.Length > 10 ? value[..10] : value;
        return DateTimeOffset.TryParseExact(text, "yyyy-MM-dd", CultureInfo.InvariantCulture, DateTimeStyles.None, out var parsed)
            ? parsed
            : DateTimeOffset.Now;
    }

    public static string FromDate(DateTimeOffset date) => date.ToString("yyyy-MM-dd", CultureInfo.InvariantCulture);
}

/// <summary>Saisie des montants : virgule ou point, analyse faite par le noyau.</summary>
public static class AmountInput
{
    private static readonly CultureInfo French = CultureInfo.GetCultureInfo("fr-FR");

    public static string Text(double value, bool emptyWhenZero = true)
        => emptyWhenZero && value == 0 ? string.Empty : value.ToString("0.##", French);

    public static double? Parse(string? text) => string.IsNullOrWhiteSpace(text) ? null : DmxFfiMethods.ParseAmountInput(text);
}

public static class Plural
{
    public static string Of(int count, string singular, string? plural = null)
        => $"{count} " + (count > 1 ? plural ?? singular + "s" : singular);
}
