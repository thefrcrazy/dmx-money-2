using System.Runtime.InteropServices;
using DmxMoney.ViewModels;

namespace DmxMoney.App;

/// <summary>
/// Rapport d'erreur fatale. Sans lui, une exception au démarrage ferme le processus sans rien
/// afficher : le détail va dans %LOCALAPPDATA%\DmxMoney\crash.log, le résumé dans une boîte de
/// dialogue Win32, qui ne dépend pas de WinUI.
/// </summary>
internal static class CrashReport
{
    private const uint MessageBoxIconError = 0x10;

    private static int reported;

    public static string LogPath { get; } = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "DmxMoney", "crash.log");

    public static void Show(Exception error)
    {
        // Une même panne peut remonter par plusieurs chemins : un seul rapport.
        if (Interlocked.Exchange(ref reported, 1) == 1)
        {
            return;
        }
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(LogPath)!);
            File.AppendAllText(LogPath, $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] DmxMoney {AppInfo.Version}\n{error}\n\n");
        }
        catch (Exception)
        {
            // Le rapport ne doit jamais masquer l'erreur d'origine.
        }
        // La CI lance l'application sans personne pour fermer une boîte de dialogue.
        if (Environment.GetEnvironmentVariable("DMXMONEY_NO_DIALOG") != "1")
        {
            MessageBoxW(
                IntPtr.Zero,
                $"DmxMoney a rencontré une erreur et doit fermer.\n\n{error.GetType().Name} : {error.Message}\n\nDétails : {LogPath}",
                "DmxMoney",
                MessageBoxIconError);
        }
    }

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern int MessageBoxW(IntPtr window, string text, string caption, uint type);
}
