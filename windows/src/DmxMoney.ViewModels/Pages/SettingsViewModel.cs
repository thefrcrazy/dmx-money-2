using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using DmxMoney.Interop;

namespace DmxMoney.ViewModels;

public sealed partial class SettingsViewModel : PageViewModel
{
    [ObservableProperty]
    private bool bridgeBusy;

    public SettingsViewModel(EngineStore store) : base(store)
    {
    }

    public override void Refresh()
    {
        OnPropertyChanged(nameof(Theme));
        OnPropertyChanged(nameof(AccentColor));
        OnPropertyChanged(nameof(IsDefaultAccent));
        OnPropertyChanged(nameof(Bridge));
        OnPropertyChanged(nameof(SecureBridge));
        OnPropertyChanged(nameof(BridgeStateLabel));
        OnPropertyChanged(nameof(BridgeStateDetail));
        OnPropertyChanged(nameof(BridgeSteps));
        OnPropertyChanged(nameof(PairingButtonLabel));
        OnPropertyChanged(nameof(QrEmptyMessage));
        OnPropertyChanged(nameof(Passkeys));
        OnPropertyChanged(nameof(UpdateAvailable));
    }

    // --- Apparence ---

    public Theme Theme
    {
        get => Store.Settings.Theme;
        set
        {
            if (value != Store.Settings.Theme)
            {
                Store.Apply(new SettingsChange.SetTheme(value));
            }
        }
    }

    public IReadOnlyList<string> AccentColors { get; } = DmxFfiMethods.AccentColors();

    public string? AccentColor => Store.Settings.AccentColor;

    public bool IsDefaultAccent => Store.Settings.AccentColor is null;

    [RelayCommand]
    private void SetAccent(string color) => Store.Apply(new SettingsChange.SetPrimaryColor(color));

    [RelayCommand]
    private void UseDefaultAccent() => Store.Apply(new SettingsChange.SetPrimaryColor("default"));

    // --- Pont PWA ---

    public bool BridgeAvailable => Store.BridgeAvailable;

    public CompanionStatus? Bridge => Store.BridgeStatus;

    public SecureBridgeInfo? SecureBridge => Store.BridgeStatus?.SecureBridge;

    public bool BridgeSwitchOn => SecureBridge?.Enabled ?? false;

    public bool IsRemoteBridge => SecureBridge?.ApiUrl?.Contains("/relay/", StringComparison.Ordinal) == true;

    public IReadOnlyList<PasskeyInfo> Passkeys => SecureBridge is null
        ? []
        : [.. SecureBridge.Passkeys.Where(passkey => passkey.RevokedAt is null)];

    public string BridgeStateLabel => !BridgeSwitchOn
        ? "Désactivé"
        : SecureBridge?.Active == true
            ? "Prêt à appairer"
            : IsRemoteBridge
                ? "Connexion Internet"
            : SecureBridge?.CertificateReady == true
                ? "Démarrage local"
                : "Préparation HTTPS";

    public string BridgeStateDetail => !BridgeSwitchOn
        ? "Activez le compagnon, puis appairez votre téléphone avec le QR."
        : IsRemoteBridge
            ? "Accès en Wi-Fi, 4G ou 5G, avec chiffrement entre vos appareils. L’ordinateur doit rester allumé et connecté."
        : SecureBridge?.Active == true
            ? "La PWA peut se connecter à l’API locale sécurisée."
            : SecureBridge?.CertificateReady == true
                ? "Le certificat est prêt, le serveur local termine son démarrage."
                : "DNS et certificat sont préparés automatiquement en arrière-plan.";

    public string LocalLabel => Bridge?.Active == true ? "Actif" : BridgeSwitchOn ? "Démarrage" : "Inactif";

    public IReadOnlyList<(string Label, string Value, bool Ready, string Icon)> BridgeSteps
    {
        get
        {
            var bridge = SecureBridge;
            var enabled = BridgeSwitchOn;
            if (IsRemoteBridge)
            {
                var connected = bridge?.Active == true;
                var encryptionReady = bridge?.ManagedCredentialReady == true;
                return
                [
                    ("Compagnon mobile", bridge?.AppUrl is not null ? "Disponible" : "En attente", bridge?.AppUrl is not null, "Globe2"),
                    ("Chiffrement entre appareils", encryptionReady ? "Prêt" : "En préparation", encryptionReady, "ShieldCheck"),
                    ("Connexion Internet", connected ? "Connectée" : "Reconnexion en cours", connected, "Wifi"),
                    ("Relais sécurisé", bridge?.Configured == true ? "Prêt" : "En préparation", bridge?.Configured == true, "Server"),
                ];
            }
            var provisioningReady = bridge?.Configured ?? false;
            var provisioning = provisioningReady ? "Prêt" : !enabled ? "En attente d’activation" : "En cours ou indisponible";
            var dns = bridge?.DnsRecordId is not null
                ? "Configuré"
                : bridge?.ManagedCredentialReady == true ? "Prêt" : enabled ? "En attente" : "En attente d’activation";
            var certificateReady = bridge?.CertificateReady ?? false;
            return
            [
                ("PWA publique", bridge?.AppUrl is not null ? "Disponible" : "En attente", bridge?.AppUrl is not null, "Globe2"),
                ("Provisionnement", provisioning, provisioningReady, "KeyRound"),
                ("DNS local", dns, bridge?.DnsRecordId is not null, "Wifi"),
                ("Certificat HTTPS", certificateReady ? "Prêt" : enabled ? "En génération" : "Absent", certificateReady, "ShieldCheck"),
                ("API locale", bridge?.ApiUrl is not null ? LocalLabel : "Non active", bridge?.Active ?? false, "Server"),
            ];
        }
    }

    public string PairingButtonLabel => SecureBridge?.Active == true
        ? "Nouveau QR"
        : BridgeSwitchOn ? IsRemoteBridge ? "Connexion…" : "Préparation HTTPS" : "Activer d’abord";

    public string QrEmptyMessage => !BridgeSwitchOn
        ? "Le QR sera disponible après activation."
        : IsRemoteBridge
            ? SecureBridge?.Active == true ? "Générez un QR pour appairer un mobile." : "Connexion Internet en cours."
        : SecureBridge?.CertificateReady != true
            ? "Certificat HTTPS en cours de génération."
            : SecureBridge?.Active != true
                ? "Serveur local en démarrage."
                : "Génère un QR pour appairer un mobile.";

    public static string PasskeyMeta(PasskeyInfo passkey)
    {
        var parts = new List<string> { $"Appairé le {DayFormat.Medium(passkey.CreatedAt[..10])}" };
        if (passkey.LastUsedAt is { Length: >= 10 } used)
        {
            parts.Add($"utilisé le {DayFormat.Medium(used[..10])}");
        }
        return string.Join(" · ", parts);
    }

    [RelayCommand]
    private async Task SetBridgeEnabledAsync(bool enabled)
    {
        BridgeBusy = true;
        await Store.PerformAsync(
            engine => engine.SetSecureBridgeEnabled(enabled),
            status =>
            {
                Store.BridgeStatus = status;
                BridgeBusy = false;
                Refresh();
            },
            message =>
            {
                Store.ErrorMessage = $"Pont sécurisé indisponible : {message}";
                BridgeBusy = false;
            });
    }

    [RelayCommand]
    private async Task RegeneratePairingAsync()
    {
        BridgeBusy = true;
        await Store.PerformAsync(
            engine => engine.RegeneratePairingToken(),
            status =>
            {
                Store.BridgeStatus = status;
                BridgeBusy = false;
                Refresh();
            },
            message =>
            {
                Store.ErrorMessage = $"Pairing impossible : {message}";
                BridgeBusy = false;
            });
    }

    [RelayCommand]
    private void CopyPairingUrl()
    {
        if (SecureBridge?.PairingUrl is { } url)
        {
            Store.Platform.CopyToClipboard(url);
            Store.ShowToast("URL copiée");
        }
    }

    [RelayCommand]
    private void RevokePasskey(PasskeyInfo passkey) => Store.Confirm(
        "Désappairer ce mobile ?",
        $"« {passkey.DeviceLabel ?? "Mobile"} » devra être appairé de nouveau pour accéder à vos données.",
        () => _ = Store.PerformAsync(
            engine => engine.RevokeMobilePasskey(passkey.Id),
            status =>
            {
                Store.BridgeStatus = status;
                Store.ShowToast("Mobile désappairé");
                Refresh();
            },
            _ => Store.ErrorMessage = "Le mobile n’a pas pu être désappairé."),
        confirmTitle: "Désappairer");

    // --- Données ---

    [RelayCommand]
    private async Task ExportDataAsync()
    {
        var today = Store.Today;
        await Store.PerformAsync(
            engine => engine.ExportBackup(),
            async content => await Store.Platform.ExportBackupAsync(content, DmxFfiMethods.BackupFileName(today)));
    }

    [RelayCommand]
    private async Task ImportDataAsync()
    {
        var picked = await Store.Platform.PickImportFileAsync();
        if (picked is null)
        {
            return;
        }
        var (content, fileName) = picked.Value;
        var extension = Path.GetExtension(fileName).TrimStart('.').ToLowerInvariant();
        Store.Present(extension is "dmx" or "json"
            ? new FormRequest.RestoreBackup(content, fileName)
            : new FormRequest.StatementImport(content, fileName));
    }

    // --- À propos ---

    public string Version => AppInfo.Version;

    public bool UpdateAvailable => Store.Platform.UpdateAvailable;

    public bool IncludePrereleases
    {
        get => Store.Platform.IncludePrereleases;
        set
        {
            if (Store.Platform.IncludePrereleases != value)
            {
                Store.Platform.IncludePrereleases = value;
                OnPropertyChanged();
                OnPropertyChanged(nameof(UpdateAvailable));
            }
        }
    }

    [RelayCommand]
    private void ShowWhatsNew() => Store.Present(new FormRequest.WhatsNew());

    [RelayCommand]
    private async Task CheckForUpdatesAsync()
    {
        await Store.Platform.CheckForUpdatesAsync();
        OnPropertyChanged(nameof(UpdateAvailable));
    }
}
