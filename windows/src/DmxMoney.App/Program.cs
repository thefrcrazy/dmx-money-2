using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Velopack;
using WinRT;

namespace DmxMoney.App;

public static class Program
{
    /// <summary>
    /// Velopack traite d'abord les arguments d'installation et de mise à jour, avant WinUI.
    /// </summary>
    [STAThread]
    public static void Main(string[] args)
    {
        AppDomain.CurrentDomain.UnhandledException += (sender, error) =>
            CrashReport.Show(error.ExceptionObject as Exception ?? new Exception(error.ExceptionObject?.ToString()));
        VelopackApp.Build().Run();
        ComWrappersSupport.InitializeComWrappers();
        // Paramètre nommé : un « _ » unique serait un vrai paramètre, et « _ = new App() » l'affecterait.
        Application.Start(callbackParams =>
        {
            var queue = DispatcherQueue.GetForCurrentThread();
            SynchronizationContext.SetSynchronizationContext(new DispatcherQueueSynchronizationContext(queue));
            try
            {
                _ = new App();
            }
            catch (Exception error)
            {
                // Sinon la boucle de messages continue sans fenêtre : un processus invisible et inutile.
                CrashReport.Show(error);
                Environment.Exit(1);
            }
        });
    }
}
