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
        "DmxMoney devient une application native : WinUI 3 sur Windows, AppKit et SwiftUI sur Mac, GTK4 sur Linux.",
        "Vos données DmxMoney 1.x sont reprises au premier lancement ; la base d'origine n'est jamais modifiée.",
        "Pont PWA sécurisé pour accéder à vos comptes depuis un mobile, avec appairage par QR et passkey.",
        "Budget, échéancier, analyses et prédictions sont calculés par le même noyau sur les trois systèmes.",
        "Installation et mises à jour légères, sans navigateur embarqué.",
    ];
}
