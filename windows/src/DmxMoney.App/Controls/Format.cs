using DmxMoney.Interop;
using DmxMoney.ViewModels;
using Microsoft.UI;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Media;
using Windows.UI;

namespace DmxMoney.App;

/// <summary>
/// Fonctions utilisées directement par les liaisons XAML (`x:Bind local:Format.…`).
/// Tous les montants et libellés viennent du noyau.
/// </summary>
public static class Format
{
    public static string Money(double amount) => ViewModels.Money.Format(amount);

    public static string Rounded(double amount) => ViewModels.Money.Rounded(amount);

    public static string Signed(double amount, TransactionType kind) => ViewModels.Money.Signed(amount, kind);

    public static string SignedIncomeOnly(double amount, TransactionType kind) => ViewModels.Money.Signed(amount, kind, false);

    public static string Percent(double value) => $"{Math.Round(value)} %";

    public static string PercentTight(double value) => $"{Math.Round(value)}%";

    public static string Short(string date) => DayFormat.Short(date);

    public static string Medium(string date) => DayFormat.Medium(date);

    public static string Numeric(string date) => DayFormat.Numeric(date);

    public static string Relative(long days) => DayFormat.Relative(days);

    public static string KindLabel(TransactionType kind) => DmxFfiMethods.TransactionTypeLabel(kind);

    public static string FrequencyLabel(Periodicity frequency) => DmxFfiMethods.PeriodicityLabel(frequency);

    public static string FrequencyFormLabel(Periodicity frequency) => DmxFfiMethods.PeriodicityFormLabel(frequency);

    public static string DueRangeLabel(ScheduledDueRange range) => DmxFfiMethods.DueRangeLabel(range);

    public static string TimeRangeLabel(TimeRange range) => DmxFfiMethods.TimeRangeLabel(range);

    public static string BudgetStateLabel(BudgetState state) => DmxFfiMethods.BudgetStateLabel(state);

    /// <summary>Légende d'un camembert : barrée et grisée quand la catégorie est masquée.</summary>
    public static Windows.UI.Text.TextDecorations Strike(bool hidden)
        => hidden ? Windows.UI.Text.TextDecorations.Strikethrough : Windows.UI.Text.TextDecorations.None;

    public static Brush LegendBrush(bool hidden, string hex)
        => hidden ? Palette.Resource("TextFillColorTertiaryBrush") : Brush(hex);

    public static string SlicePercent(bool hidden, double percentage) => hidden ? string.Empty : Percent(percentage);

    public static Brush KindTint(TransactionType kind) => kind switch
    {
        TransactionType.Income => Tint("#10b981"),
        TransactionType.Transfer => Tint("#6366f1"),
        _ => Tint("#ef4444"),
    };

    public static string KindGlyph(TransactionType kind) => kind switch
    {
        TransactionType.Income => "TrendingUp",
        TransactionType.Transfer => "ArrowRightLeft",
        _ => "TrendingDown",
    };

    /// <summary>« 24 déc. 2026 • Compte courant → Livret » ou « • Cadeaux ».</summary>
    public static string FakeDetails(string date, string source, string? destination, string? category)
    {
        var parts = new List<string> { Medium(date), source };
        if (destination is { Length: > 0 })
        {
            parts.Add($"→ {destination}");
        }
        else if (category is { Length: > 0 })
        {
            parts.Add(category);
        }
        return string.Join(" • ", parts);
    }

    /// <summary>« Compte courant • 4 mois observés • 82,30 € ce mois-ci ».</summary>
    public static string SuggestionDetails(string accountName, uint monthCount, double currentMonthSpent)
    {
        var parts = new List<string> { accountName, $"{monthCount} mois {(monthCount > 1 ? "observés" : "observé")}" };
        if (currentMonthSpent > 0)
        {
            parts.Add($"{Money(currentMonthSpent)} ce mois-ci");
        }
        return string.Join(" • ", parts);
    }

    public static string ScheduledSuggestionDetails(string accountName, Periodicity frequency, uint occurrences, string nextDate)
        => string.Join(" • ", accountName, FrequencyLabel(frequency), Count((int)occurrences, "occurrence"), $"prochaine le {Short(nextDate)}");

    public static string BackupDate(string? date) => date is null ? string.Empty : $"Sauvegarde du {date}";

    public static string VersionLabel(string version) => $"DmxMoney {version}";

    public static string PasskeyName(string? label) => label ?? "Mobile";

    public static string PasskeyMeta(string createdAt, string? lastUsedAt)
    {
        var parts = new List<string> { $"Appairé le {Medium(createdAt[..Math.Min(10, createdAt.Length)])}" };
        if (lastUsedAt is { Length: >= 10 } used)
        {
            parts.Add($"utilisé le {Medium(used[..10])}");
        }
        return string.Join(" · ", parts);
    }

    public static string LowLabel(double low) => $"Point bas {Money(low)}";

    public static string CloseLabel(double value) => $"Fin de journée {Money(value)}";

    public static Brush LegendTextBrush(bool hidden)
        => hidden ? Palette.Resource("TextFillColorSecondaryBrush") : Palette.Resource("TextFillColorPrimaryBrush");

    public static string Upper(string text) => text.ToUpperInvariant();

    public static string Count(int count, string singular) => Plural.Of(count, singular);

    public static string Ratio(int visible, int total) => $"{visible} / {total}";

    public static Brush Brush(string hex) => Palette.ToBrush(hex, Colors.Gray);

    public static Brush Tint(string hex)
    {
        var color = Palette.ToColor(hex, Colors.Gray);
        return new SolidColorBrush(Color.FromArgb(34, color.R, color.G, color.B));
    }

    public static Brush KindBrush(TransactionType kind) => Palette.ForKind(kind);

    public static Brush AmountBrush(double amount)
        => amount < 0 ? Palette.Expense : Palette.Resource("TextFillColorPrimaryBrush");

    public static Brush IncomeExpenseBrush(TransactionType kind)
        => kind == TransactionType.Income ? Palette.Income : Palette.Expense;

    public static Brush PositiveBrush(double amount) => amount >= 0 ? Palette.Income : Palette.Expense;

    public static Visibility Visible(bool flag) => flag ? Visibility.Visible : Visibility.Collapsed;

    public static Visibility Hidden(bool flag) => flag ? Visibility.Collapsed : Visibility.Visible;

    public static Visibility VisibleIfText(string? text)
        => string.IsNullOrEmpty(text) ? Visibility.Collapsed : Visibility.Visible;

    public static Visibility VisibleIfAny(int count) => count > 0 ? Visibility.Visible : Visibility.Collapsed;

    public static Visibility VisibleIfNone(int count) => count > 0 ? Visibility.Collapsed : Visibility.Visible;

    public static string CheckGlyph(bool checkedState) => checkedState ? "CheckCircle2" : "Circle";

    public static Brush CheckBrush(bool checkedState) => checkedState ? Palette.Income : Palette.Resource("TextFillColorTertiaryBrush");

    /// <summary>« 2 enveloppes · 1 échéance liée ».</summary>
    public static string Envelopes(int envelopes, uint scheduled)
        => $"{Plural.Of(envelopes, "enveloppe")} · {Plural.Of((int)scheduled, "échéance")} {(scheduled > 1 ? "liées" : "liée")}";

    /// <summary>Pastille « OK » : ni dépassement, ni catégorie hors budget.</summary>
    public static Visibility OkVisible(bool overBudget, bool unbudgeted)
        => overBudget || unbudgeted ? Visibility.Collapsed : Visibility.Visible;
}
