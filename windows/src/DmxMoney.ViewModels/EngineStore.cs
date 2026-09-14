using System.ComponentModel;
using CommunityToolkit.Mvvm.ComponentModel;
using DmxMoney.Interop;

namespace DmxMoney.ViewModels;

/// <summary>
/// État partagé de l'application : moteur Rust, réglages, comptes, catégories et filtre global.
/// Aucun calcul métier ici : tout vient du noyau.
/// </summary>
public sealed partial class EngineStore : ObservableObject, IDisposable
{
    private sealed class Listener(EngineStore store) : EngineListener
    {
        public void DataChanged(long dataVersion) => store.Dispatch(() =>
        {
            if (dataVersion != store.DataVersion)
            {
                store.Reload();
            }
        });

        public void BridgeStatusChanged() => store.Dispatch(store.RefreshBridgeStatus);

        // Appelé depuis le fil du pont, avant que le noyau n'interprète la phrase.
        public string? RephraseAssistantRequest(string text) => store.AssistantRewriter?.Invoke(text);
    }

    private readonly Listener listener;
    private CancellationTokenSource? toastCancellation;

    public DmxEngine Engine { get; }

    public IPlatformServices Platform { get; set; }

    /// <summary>Pont PWA autorisé pour ce lancement (désactivé sur un dossier de test).</summary>
    public bool BridgeEnabled { get; private set; } = true;

    /// <summary>Renvoi vers le fil d'interface ; remplacé par l'application WinUI.</summary>
    public Action<Action> Dispatch { get; set; } = action => action();

    /// <summary>
    /// Reformulation par un modèle local d'une demande envoyée à l'assistant, hors du fil d'interface.
    /// Sans modèle (par défaut), le noyau interprète la phrase telle quelle.
    /// </summary>
    public Func<string, string?>? AssistantRewriter { get; set; }

    public string Today => DmxFfiMethods.Today();

    [ObservableProperty]
    private long dataVersion;

    [ObservableProperty]
    private AppSettings settings = null!;

    [ObservableProperty]
    private IReadOnlyList<Account> accounts = [];

    [ObservableProperty]
    private IReadOnlyList<Category> categories = [];

    [ObservableProperty]
    private BalanceSummary balances = new(0, 0);

    /// <summary>Incrémenté à chaque changement de données ou de filtre : les pages se recalculent.</summary>
    [ObservableProperty]
    private int revision;

    [ObservableProperty]
    private AppRoute route = AppRoute.Dashboard;

    [ObservableProperty]
    private FormRequest? form;

    [ObservableProperty]
    private ConfirmRequest? confirmation;

    [ObservableProperty]
    private string? toast;

    [ObservableProperty]
    private string? errorMessage;

    [ObservableProperty]
    private CompanionStatus? bridgeStatus;

    [ObservableProperty]
    private ProcessDueResult? dueResult;

    private IReadOnlyList<string> selectedAccountIds = [];

    /// <summary>Comptes cochés dans le filtre global (vide = tous les comptes).</summary>
    public IReadOnlyList<string> SelectedAccountIds
    {
        get => selectedAccountIds;
        set
        {
            if (selectedAccountIds.SequenceEqual(value))
            {
                return;
            }
            selectedAccountIds = value;
            OnPropertyChanged();
            OnPropertyChanged(nameof(IsFiltering));
            BumpRevision();
        }
    }

    public bool IsFiltering => SelectedAccountIds.Count > 0;

    public EngineStore(DmxEngine engine, IPlatformServices? platform = null)
    {
        Engine = engine;
        Platform = platform ?? new NullPlatformServices();
        listener = new Listener(this);
        Engine.SetListener(listener);
        Reload();
    }

    /// <summary>
    /// Ouvre la base de l'utilisateur et reprend au besoin celle de DmxMoney 1.x.
    /// <c>DMXMONEY_DATA_DIR</c> permet de travailler sur un dossier de test.
    /// </summary>
    public static EngineStore Open(IPlatformServices? platform = null)
    {
        var overridden = Environment.GetEnvironmentVariable("DMXMONEY_DATA_DIR");
        if (!string.IsNullOrWhiteSpace(overridden))
        {
            Directory.CreateDirectory(overridden);
            return new EngineStore(DmxEngine.Open(overridden, []), platform) { BridgeEnabled = false };
        }
        var directory = DataDirectory();
        Directory.CreateDirectory(directory);
        return new EngineStore(DmxEngine.Open(directory, DmxFfiMethods.DefaultLegacyDatabasePaths()), platform);
    }

    public static string DataDirectory() => Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
        "com.dmxmoney.app");

    // --- Rechargement ---

    public void Reload()
    {
        try
        {
            var version = Engine.DataVersion();
            Settings = Engine.Settings();
            Accounts = Engine.AccountsList();
            Categories = Engine.Categories();
            var known = Accounts.Select(account => account.Id).ToHashSet();
            var filtered = SelectedAccountIds.Where(known.Contains).ToList();
            if (filtered.Count != SelectedAccountIds.Count)
            {
                selectedAccountIds = filtered;
                OnPropertyChanged(nameof(SelectedAccountIds));
                OnPropertyChanged(nameof(IsFiltering));
            }
            DataVersion = version;
            BumpRevision();
        }
        catch (Exception error)
        {
            ErrorMessage = Message(error);
        }
    }

    private void BumpRevision()
    {
        try
        {
            Balances = Engine.Dashboard([.. SelectedAccountIds], Today).Balances;
        }
        catch (Exception)
        {
            // Les soldes de la barre d'outils ne doivent jamais bloquer l'interface.
        }
        Revision++;
    }

    /// <summary>Crée les opérations des échéances arrivées à terme (lancement, retour au premier plan).</summary>
    public void ProcessDueScheduled()
    {
        var today = Today;
        _ = PerformAsync(engine => engine.ProcessDueScheduled(today), result =>
        {
            if (result.CreatedTransactions > 0 || result.UpdatedScheduled > 0 || result.DeletedScheduled > 0)
            {
                DueResult = result;
            }
        });
    }

    // --- Exécution ---

    /// <summary>Lecture synchrone ; en cas d'échec l'erreur est publiée et <c>null</c> renvoyé.</summary>
    public T? Read<T>(Func<DmxEngine, T> work)
    {
        try
        {
            return work(Engine);
        }
        catch (Exception error)
        {
            ErrorMessage = Message(error);
            return default;
        }
    }

    /// <summary>Écriture synchrone ; renvoie le message d'erreur à afficher, ou <c>null</c>.</summary>
    public string? Attempt(Action<DmxEngine> work)
    {
        try
        {
            work(Engine);
            Reload();
            return null;
        }
        catch (Exception error)
        {
            return Message(error);
        }
    }

    /// <summary>Écriture avec message de réussite ; l'erreur éventuelle est affichée.</summary>
    public void Run(Action<DmxEngine> work, string? success = null)
    {
        var message = Attempt(work);
        if (message is not null)
        {
            ErrorMessage = message;
        }
        else if (success is not null)
        {
            ShowToast(success);
        }
    }

    /// <summary>Travail long (pont, import, restauration) hors du fil d'interface.</summary>
    public async Task PerformAsync<T>(Func<DmxEngine, T> work, Action<T>? onSuccess = null, Action<string>? onFailure = null)
    {
        var engine = Engine;
        try
        {
            var value = await Task.Run(() => work(engine)).ConfigureAwait(false);
            Dispatch(() =>
            {
                Reload();
                onSuccess?.Invoke(value);
            });
        }
        catch (Exception error)
        {
            var message = Message(error);
            Dispatch(() =>
            {
                if (onFailure is not null)
                {
                    onFailure(message);
                }
                else
                {
                    ErrorMessage = message;
                }
            });
        }
    }

    public void Apply(SettingsChange change) => Run(engine => engine.ApplySettingsChange(change));

    // --- Présentation ---

    public void Present(FormRequest request) => Form = request;

    public void Confirm(string title, string message, Action action, string confirmTitle = "Supprimer", bool destructive = true)
        => Confirmation = new ConfirmRequest(title, message, confirmTitle, destructive, action);

    public void ShowToast(string message)
    {
        Toast = message;
        toastCancellation?.Cancel();
        var cancellation = new CancellationTokenSource();
        toastCancellation = cancellation;
        _ = Task.Delay(TimeSpan.FromSeconds(2.5), cancellation.Token).ContinueWith(task =>
        {
            if (!task.IsCanceled)
            {
                Dispatch(() =>
                {
                    if (Toast == message)
                    {
                        Toast = null;
                    }
                });
            }
        }, TaskScheduler.Default);
    }

    // --- Filtre de comptes ---

    public bool IsSelected(string accountId) => SelectedAccountIds.Count == 0 || SelectedAccountIds.Contains(accountId);

    public void ToggleAccountFilter(string accountId)
    {
        var selection = SelectedAccountIds.ToList();
        if (!selection.Remove(accountId))
        {
            selection.Add(accountId);
        }
        SelectedAccountIds = selection.Count == Accounts.Count ? [] : selection;
    }

    public void ClearAccountFilter() => SelectedAccountIds = [];

    public Account? AccountById(string? id) => id is null ? null : Accounts.FirstOrDefault(account => account.Id == id);

    public Category? CategoryById(string? id) => id is null ? null : Categories.FirstOrDefault(category => category.Id == id);

    /// <summary>Catégories choisissables (sans « Virement », réservée aux virements).</summary>
    public IReadOnlyList<Category> SelectableCategories => Categories.Where(category => category.Id != "transfer").ToList();

    // --- Pont PWA ---

    public bool BridgeAvailable => BridgeEnabled && Engine.BridgeSupported();

    public void RefreshBridgeStatus()
    {
        if (!BridgeAvailable)
        {
            return;
        }
        _ = PerformAsync(engine => engine.BridgeStatus(), status => BridgeStatus = status, _ => { });
    }

    public void StartBridge(string? assetsDirectory)
    {
        if (!BridgeAvailable)
        {
            return;
        }
        _ = PerformAsync<object?>(engine =>
        {
            engine.StartBridge(assetsDirectory);
            return null;
        }, _ => RefreshBridgeStatus(), _ => { });
    }

    // --- Erreurs ---

    public static string Message(Exception error) => error switch
    {
        DmxException.Database e => e.message,
        DmxException.Validation e => e.message,
        DmxException.NotFound e => e.message,
        DmxException.Import e => e.message,
        DmxException.Io e => e.message,
        DmxException.Bridge e => e.message,
        DmxException.Unsupported e => e.message,
        _ => error.Message,
    };

    public void Dispose()
    {
        toastCancellation?.Cancel();
        Engine.StopBridge();
        Engine.Dispose();
    }
}
