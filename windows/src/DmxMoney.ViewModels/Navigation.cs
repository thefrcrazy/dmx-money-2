using DmxMoney.Interop;

namespace DmxMoney.ViewModels;

public enum AppRoute
{
    Dashboard,
    Accounts,
    Transactions,
    Budget,
    Scheduled,
    Analytics,
    Predictions,
    Categories,
    Settings,
}

public sealed record SidebarSection(string Title, AppRoute[] Routes);

public static class AppRoutes
{
    public static readonly AppRoute[] All = Enum.GetValues<AppRoute>();

    public static readonly SidebarSection[] SidebarSections =
    [
        new("Général", [AppRoute.Dashboard, AppRoute.Accounts, AppRoute.Transactions]),
        new("Finances", [AppRoute.Budget, AppRoute.Scheduled]),
        new("Analyses", [AppRoute.Analytics, AppRoute.Predictions]),
    ];

    public static readonly AppRoute[] FooterRoutes = [AppRoute.Categories, AppRoute.Settings];

    public static string Title(this AppRoute route) => route switch
    {
        AppRoute.Dashboard => "Vue d'ensemble",
        AppRoute.Accounts => "Mes Comptes",
        AppRoute.Transactions => "Journal",
        AppRoute.Budget => "Budget",
        AppRoute.Scheduled => "Échéancier",
        AppRoute.Analytics => "Analyses",
        AppRoute.Predictions => "Prédictions",
        AppRoute.Categories => "Catégories",
        _ => "Paramètres",
    };

    /// <summary>Nom de l'icône Lucide, identique aux autres plateformes.</summary>
    public static string Icon(this AppRoute route) => route switch
    {
        AppRoute.Dashboard => "LayoutDashboard",
        AppRoute.Accounts => "Wallet",
        AppRoute.Transactions => "Receipt",
        AppRoute.Budget => "Calculator",
        AppRoute.Scheduled => "CalendarClock",
        AppRoute.Analytics => "PieChart",
        AppRoute.Predictions => "TrendingUp",
        AppRoute.Categories => "Tag",
        _ => "Settings",
    };

    public static bool UsesAccountFilter(this AppRoute route) => route is AppRoute.Dashboard or AppRoute.Transactions
        or AppRoute.Budget or AppRoute.Scheduled or AppRoute.Analytics or AppRoute.Predictions;

    public static bool ShowsBalances(this AppRoute route) => route is AppRoute.Dashboard or AppRoute.Transactions;
}

/// <summary>Formulaires demandés par les pages ; l'hôte les présente en boîte de dialogue.</summary>
public abstract record FormRequest
{
    public sealed record AccountForm(string? Id) : FormRequest;

    public sealed record AccountGroups : FormRequest;

    public sealed record TransactionForm(string? Id) : FormRequest;

    public sealed record CategoryForm(string? Id) : FormRequest;

    public sealed record BudgetForm(string? Id, string? CategoryId = null) : FormRequest;

    public sealed record ScheduledForm(string? Id) : FormRequest;

    public sealed record FakeTransactionForm(string? Id) : FormRequest;

    public sealed record BudgetSuggestions : FormRequest;

    public sealed record ScheduledSuggestions : FormRequest;

    public sealed record RestoreBackup(string Content, string FileName) : FormRequest;

    public sealed record StatementImport(string Content, string FileName) : FormRequest;

    public sealed record WhatsNew : FormRequest;
}

/// <summary>Choix du type d'opération dans un formulaire.</summary>
public sealed record KindOption(TransactionType Value, string Label);

/// <summary>Choix d'une fréquence dans un formulaire.</summary>
public sealed record PeriodicityOption(Periodicity Value, string Label);

public sealed record ConfirmRequest(string Title, string Message, string ConfirmTitle, bool Destructive, Action Action);

/// <summary>Services propres à la plateforme (sélecteurs de fichiers, presse-papiers, mises à jour).</summary>
public interface IPlatformServices
{
    Task ExportBackupAsync(string content, string suggestedFileName);

    Task<(string Content, string FileName)?> PickImportFileAsync();

    void CopyToClipboard(string text);

    bool UpdateAvailable { get; }

    Task CheckForUpdatesAsync();
}

public sealed class NullPlatformServices : IPlatformServices
{
    public bool UpdateAvailable => false;

    public Task ExportBackupAsync(string content, string suggestedFileName) => Task.CompletedTask;

    public Task<(string Content, string FileName)?> PickImportFileAsync() => Task.FromResult<(string, string)?>(null);

    public void CopyToClipboard(string text)
    {
    }

    public Task CheckForUpdatesAsync() => Task.CompletedTask;
}
