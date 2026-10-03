using System.Reflection;
using DmxMoney.Interop;

namespace DmxMoney.ViewModels;

public static class AppInfo
{
    public static string Version { get; } =
        typeof(AppInfo).Assembly.GetCustomAttribute<AssemblyInformationalVersionAttribute>()?.InformationalVersion.Split('+')[0]
        ?? DmxFfiMethods.CoreVersion();

    public static IReadOnlyList<string> ReleaseNotes { get; } =
    [
        "Journal : réutilisation des cellules et limitation des rechargements pendant le défilement.",
        "Les lectures du statut du compagnon ne recalculent plus inutilement le journal inchangé.",
        "Les vues dépendantes du jour continuent de s'actualiser après minuit.",
        "Journal PWA mobile virtualisé, avec conservation de la sélection et de la ligne focalisée.",
        "Pré-version 2.1.0 RC2 : les mises à jour stables restent séparées.",
    ];
}
