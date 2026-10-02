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
        "Compagnon Internet unique : activez-le puis scannez le QR pour appairer votre téléphone.",
        "La PWA Cloudflare Pages transmet les modifications au bureau par un relais chiffré en Wi-Fi, 4G ou 5G.",
        "Le serveur local, les certificats DNS et les actions de migration ont été retirés.",
        "La connexion mobile reprend après réouverture, avec une passkey et un verrouillage volontaire.",
        "Pré-version 2.1.0 RC1 : les mises à jour stables restent séparées.",
    ];
}
