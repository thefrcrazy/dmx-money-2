using DmxMoney.Interop;
using DmxMoney.ViewModels;

namespace DmxMoney.Tests;

public sealed class CompanionSettingsTests
{
    private static SecureBridgeInfo Remote(bool active) => new(
        Enabled: true, Configured: true, Active: active, Domain: null,
        AppUrl: "https://companion.example.com", LocalHost: null, DeviceId: "desktop",
        ApiUrl: "https://companion.example.com/relay/0123456789abcdef0123456789abcdef", Port: null,
        PairingUrl: null, PairingTokenExpiresAt: null, CertificateExpiresAt: null,
        CertificateReady: false, DnsRecordId: null, DnsLastUpdatedAt: null,
        ManagedServiceUrl: "https://companion.example.com", ManagedCredentialReady: true,
        Passkeys: [], LastError: null, Degraded: false);

    [Fact]
    public void RemoteCompanionDoesNotWaitForALocalCertificate()
    {
        using var store = new EngineStore(DmxEngine.OpenInMemory());
        store.BridgeStatus = new CompanionStatus(true, true, null, null, null, 1, Remote(true));
        using var settings = new SettingsViewModel(store);

        Assert.Equal("Prêt à appairer", settings.BridgeStateLabel);
        Assert.Equal("Nouveau QR", settings.PairingButtonLabel);
        Assert.Contains("appairer", settings.QrEmptyMessage);
        Assert.DoesNotContain(settings.BridgeSteps, step => step.Label.Contains("DNS") || step.Label.Contains("Certificat"));
        Assert.All(settings.BridgeSteps, step => Assert.True(step.Ready));
    }

    [Fact]
    public void RemoteCompanionShowsReconnectionWhenTheDesktopSocketIsOffline()
    {
        using var store = new EngineStore(DmxEngine.OpenInMemory());
        store.BridgeStatus = new CompanionStatus(true, true, null, null, null, 1, Remote(false));
        using var settings = new SettingsViewModel(store);

        Assert.Equal("Connexion Internet", settings.BridgeStateLabel);
        Assert.Equal("Connexion…", settings.PairingButtonLabel);
        Assert.Equal("Connexion Internet en cours.", settings.QrEmptyMessage);
        var connection = Assert.Single(settings.BridgeSteps, step => step.Label == "Connexion Internet");
        Assert.False(connection.Ready);
        Assert.Equal("Reconnexion en cours", connection.Value);
    }

    [Fact]
    public void DisabledCompanionOffersOnlyInternetActivation()
    {
        using var store = new EngineStore(DmxEngine.OpenInMemory());
        store.BridgeStatus = new CompanionStatus(true, true, null, null, null, 1, Remote(false) with { Enabled = false });
        using var settings = new SettingsViewModel(store);

        Assert.Equal("Désactivé", settings.BridgeStateLabel);
        Assert.Contains("4G ou 5G", settings.BridgeStateDetail);
        Assert.Equal("Activer d’abord", settings.PairingButtonLabel);
        Assert.Equal("Le QR sera disponible après activation.", settings.QrEmptyMessage);
        var connection = Assert.Single(settings.BridgeSteps, step => step.Label == "Connexion Internet");
        Assert.False(connection.Ready);
        Assert.Equal("Désactivée", connection.Value);
    }

    [Fact]
    public void NewInstallationHasOneCompanionFlowWithoutEndpointConfiguration()
    {
        using var store = new EngineStore(DmxEngine.OpenInMemory());
        store.BridgeStatus = new CompanionStatus(true, false, null, null, null, 0, null);
        using var settings = new SettingsViewModel(store);

        Assert.False(settings.BridgeSwitchOn);
        Assert.Equal("Désactivé", settings.BridgeStateLabel);
        Assert.Empty(settings.Passkeys);
        Assert.Contains("Face ID", settings.QrInstructions);
        Assert.Contains("Internet", settings.QrInstructions);
        Assert.All(settings.BridgeSteps, step => Assert.False(step.Ready));
        Assert.DoesNotContain(settings.BridgeSteps, step => step.Label.Contains("DNS") || step.Label.Contains("Certificat"));
        Assert.Null(store.Confirmation);
    }

    [Fact]
    public async Task DisablingCompanionDoesNotRequireASetupConfirmation()
    {
        using var store = new EngineStore(DmxEngine.OpenInMemory());
        store.BridgeStatus = new CompanionStatus(true, true, null, null, null, 1, Remote(true));
        using var settings = new SettingsViewModel(store);

        await settings.SetBridgeEnabledCommand.ExecuteAsync(false);

        Assert.Null(store.Confirmation);
        Assert.False(settings.BridgeBusy);
        Assert.False(settings.BridgeSwitchOn);
        Assert.Equal("Désactivé", settings.BridgeStateLabel);
    }
}
