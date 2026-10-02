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
        OnPropertyChanged(nameof(HasLegacyBridge));
        OnPropertyChanged(nameof(NeedsHostedPwaUpdate));
        OnPropertyChanged(nameof(CompanionMigrationRequired));
        OnPropertyChanged(nameof(CompanionMigrationLabel));
        OnPropertyChanged(nameof(CompanionMigrationDetail));
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

    // --- Pont PWA ---

    public bool BridgeAvailable => Store.BridgeAvailable;

    public CompanionStatus? Bridge => Store.BridgeStatus;

    public SecureBridgeInfo? SecureBridge => Store.BridgeStatus?.SecureBridge;

    public bool BridgeSwitchOn => SecureBridge?.Enabled ?? false;

    public bool IsRemoteBridge => SecureBridge?.ApiUrl?.Contains("/relay/", StringComparison.Ordinal) == true;

    public bool HasLegacyBridge => !IsRemoteBridge && SecureBridge?.LocalHost is not null;

    public bool NeedsHostedPwaUpdate =>
        Uri.TryCreate(SecureBridge?.ApiUrl, UriKind.Absolute, out var api)
        && HasHttpsOrigin(api, "dmxmoney-remote-relay.qm7ws5twn7.workers.dev")
        && api.AbsolutePath.StartsWith("/relay/", StringComparison.Ordinal)
        && (!Uri.TryCreate(SecureBridge?.AppUrl, UriKind.Absolute, out var app)
            || !HasHttpsOrigin(app, "dmxmoney-companion.pages.dev"));

    public bool CompanionMigrationRequired => HasLegacyBridge || NeedsHostedPwaUpdate;

    public string CompanionMigrationLabel => HasLegacyBridge ? "Passer à l’accès Internet" : "Mettre à jour le compagnon";

    public string CompanionMigrationDetail => HasLegacyBridge
        ? "Votre ancien compagnon utilise le réseau local. L’accès en 4G ou 5G nécessite le compagnon Internet."
        : "Votre compagnon Internet utilise l’ancienne page. Mettez-le à jour pour ouvrir la PWA Cloudflare Pages.";

    private static bool HasHttpsOrigin(Uri uri, string host) =>
        uri.Scheme == Uri.UriSchemeHttps && uri.Host.Equals(host, StringComparison.OrdinalIgnoreCase)
        && uri.Port == 443 && uri.UserInfo.Length == 0;

    public string QrInstructions => !HasLegacyBridge
        ? "Scannez ce QR, puis validez avec Face ID, Touch ID ou le verrouillage du téléphone. Vos modifications seront envoyées à DmxMoney sur cet ordinateur par Internet. Un QR par appareil."
        : "Ce QR utilise l’ancien accès local, disponible sur le même réseau que cet ordinateur. Passez à l’accès Internet pour utiliser la 4G ou la 5G.";

    public IReadOnlyList<PasskeyInfo> Passkeys => SecureBridge is null
        ? []
        : [.. SecureBridge.Passkeys.Where(passkey => passkey.RevokedAt is null)];

    public string BridgeStateLabel => !BridgeSwitchOn
        ? "Désactivé"
        : !IsRemoteBridge
            ? "Accès local hérité"
        : SecureBridge?.Active == true
            ? "Prêt à appairer"
            : "Connexion Internet";

    public string BridgeStateDetail => !BridgeSwitchOn
        ? "Activez l’accès Internet pour modifier les données de cet ordinateur depuis votre téléphone, en Wi-Fi, 4G ou 5G, puis scannez le QR."
        : IsRemoteBridge
            ? "Les modifications de votre téléphone passent par le relais chiffré jusqu’à cet ordinateur, en Wi-Fi, 4G ou 5G. DmxMoney doit rester ouvert sur cet ordinateur allumé et connecté à Internet."
            : "Ancien accès local : le téléphone doit être sur le même réseau que cet ordinateur. Cet accès ne fonctionne pas en 4G ou 5G.";

    public string LocalLabel => Bridge?.Active == true ? "Actif" : BridgeSwitchOn ? "Démarrage" : "Inactif";

    public IReadOnlyList<(string Label, string Value, bool Ready, string Icon)> BridgeSteps
    {
        get
        {
            var bridge = SecureBridge;
            var enabled = BridgeSwitchOn;
            if (!HasLegacyBridge)
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
    private void SetBridgeEnabled(bool enabled)
    {
        if (BridgeBusy)
        {
            return;
        }
        if (enabled && CompanionMigrationRequired)
        {
            Store.Confirm(
                HasLegacyBridge ? "Passer à l’accès Internet ?" : "Mettre à jour le compagnon ?",
                "Synchronisez d’abord les saisies en attente dans l’ancienne PWA. Vos mobiles devront ensuite être appairés avec un nouveau QR ; les anciennes sessions seront révoquées.",
                () => _ = ApplyBridgeEnabledAsync(true),
                confirmTitle: HasLegacyBridge ? "Passer à Internet" : "Mettre à jour",
                destructive: false);
        }
        else
        {
            _ = ApplyBridgeEnabledAsync(enabled);
        }
    }

    [RelayCommand]
    private void MigrateBridge() => SetBridgeEnabled(true);

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
