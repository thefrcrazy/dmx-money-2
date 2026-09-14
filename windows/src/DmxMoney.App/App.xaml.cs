using DmxMoney.Interop;
using DmxMoney.ViewModels;
using Microsoft.UI.Xaml;

namespace DmxMoney.App;

public partial class App : Application
{
    public static EngineStore Store { get; private set; } = null!;

    public static ShellViewModel Shell { get; private set; } = null!;

    public static MainWindow? Window { get; private set; }

    public App()
    {
        // Abonné avant InitializeComponent, pour couvrir aussi le chargement du XAML. Le message de
        // WinUI (args.Message) dit souvent ce que l'exception native ne dit pas.
        UnhandledException += (sender, args) => CrashReport.Show(args.Exception, args.Message);
        InitializeComponent();
        DebugSettings.XamlResourceReferenceFailed += (sender, args) => CrashReport.Note($"ressource XAML introuvable : {args.Message}");
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        try
        {
            Store = EngineStore.Open();
        }
        catch (Exception error)
        {
            // Sans base, l'application ne peut rien afficher : message puis arrêt.
            ShowFatal(EngineStore.Message(error));
            return;
        }

        var window = new MainWindow();
        Window = window;
        Store.Dispatch = action =>
        {
            if (!window.DispatcherQueue.TryEnqueue(() => action()))
            {
                action();
            }
        };
        Store.Platform = new WindowsPlatformServices(window);
        Shell = new ShellViewModel(Store);
        window.Attach(Shell);
        window.Activate();
        Shell.Start(PwaAssetsDirectory());
    }

    /// <summary>Dossier de la PWA embarquée, servi par le pont local quand il est présent.</summary>
    private static string? PwaAssetsDirectory()
    {
        var directory = Path.Combine(AppContext.BaseDirectory, "Assets", "pwa");
        return Directory.Exists(directory) ? directory : null;
    }

    private static void ShowFatal(string message)
    {
        var window = new Window { Title = "DmxMoney" };
        window.Content = new Microsoft.UI.Xaml.Controls.TextBlock
        {
            Text = $"Impossible d'ouvrir la base DmxMoney :\n{message}",
            Margin = new Thickness(24),
            TextWrapping = TextWrapping.Wrap,
        };
        window.Activate();
    }
}
