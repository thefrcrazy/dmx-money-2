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
        OnPropertyChanged(nameof(SecureBridge));
        OnPropertyChanged(nameof(QrInstructions));
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

    // --- Compagnon Internet ---

    public bool BridgeAvailable => Store.BridgeAvailable;

    public SecureBridgeInfo? SecureBridge => Store.BridgeStatus?.SecureBridge;

    public bool BridgeSwitchOn => SecureBridge?.Enabled ?? false;

    public string QrInstructions => "Scannez ce QR, puis validez avec Face ID, Touch ID ou le verrouillage du téléphone. Vos modifications seront envoyées à DmxMoney sur cet ordinateur par Internet. Un QR par appareil.";

    public IReadOnlyList<PasskeyInfo> Passkeys => SecureBridge is null
        ? []
        : [.. SecureBridge.Passkeys.Where(passkey => passkey.RevokedAt is null)];

    public string BridgeStateLabel => !BridgeSwitchOn
        ? "Désactivé"
        : SecureBridge?.Active == true
            ? "Prêt à appairer"
            : "Connexion Internet";

    public string BridgeStateDetail => !BridgeSwitchOn
        ? "Activez l’accès Internet pour modifier les données de cet ordinateur depuis votre téléphone, en Wi-Fi, 4G ou 5G, puis scannez le QR."
        : "Les modifications de votre téléphone passent par le relais chiffré jusqu’à cet ordinateur, en Wi-Fi, 4G ou 5G. DmxMoney doit rester ouvert sur cet ordinateur allumé et connecté à Internet.";

    public IReadOnlyList<(string Label, string Value, bool Ready, string Icon)> BridgeSteps
    {
        get
        {
            var bridge = SecureBridge;
            var connected = BridgeSwitchOn && bridge?.Active == true;
            var encryptionReady = bridge?.ManagedCredentialReady == true;
            return
            [
                ("Compagnon mobile", bridge?.AppUrl is not null ? "Disponible" : "En attente", bridge?.AppUrl is not null, "Globe2"),
                ("Chiffrement entre appareils", encryptionReady ? "Prêt" : "En préparation", encryptionReady, "ShieldCheck"),
                ("Connexion Internet", connected ? "Connectée" : BridgeSwitchOn ? "Reconnexion en cours" : "Désactivée", connected, "Wifi"),
                ("Relais sécurisé", bridge?.Configured == true ? "Prêt" : "En préparation", bridge?.Configured == true, "Server"),
            ];
        }
    }

    public string PairingButtonLabel => !BridgeSwitchOn
        ? "Activer d’abord"
        : SecureBridge?.Active == true ? "Nouveau QR" : "Connexion…";

    public string QrEmptyMessage => !BridgeSwitchOn
        ? "Le QR sera disponible après activation."
        : SecureBridge?.Active == true ? "Générez un QR pour appairer un mobile." : "Connexion Internet en cours.";

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
        if (BridgeBusy)
        {
            return;
        }
        await ApplyBridgeEnabledAsync(enabled);
    }

    private async Task ApplyBridgeEnabledAsync(bool enabled)
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
                Store.ErrorMessage = $"Compagnon Internet indisponible : {message}";
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
        try
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
        catch (Exception error) { Store.ErrorMessage = EngineStore.Message(error); }
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
