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

        Assert.True(settings.IsRemoteBridge);
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
    public void LegacyCompanionRequiresAnExplicitMigrationBeforeRevokingMobileAccess()
    {
        using var store = new EngineStore(DmxEngine.OpenInMemory());
        var legacy = Remote(true) with
        {
            LocalHost = "legacy.example.com",
            ApiUrl = "https://legacy.example.com:8443",
            CertificateReady = true,
        };
        store.BridgeStatus = new CompanionStatus(true, true, null, null, null, 1, legacy);
        using var settings = new SettingsViewModel(store);

        Assert.True(settings.HasLegacyBridge);
        Assert.Equal("Accès local hérité", settings.BridgeStateLabel);
        Assert.Contains("ne fonctionne pas en 4G ou 5G", settings.BridgeStateDetail);
        Assert.Contains("même réseau", settings.QrInstructions);
        settings.MigrateBridgeCommand.Execute(null);
        Assert.NotNull(store.Confirmation);
        Assert.Contains("saisies en attente", store.Confirmation.Message);
        Assert.Contains("révoquées", store.Confirmation.Message);
        Assert.False(store.Confirmation.Destructive);
        Assert.False(settings.BridgeBusy);
        Assert.Same(legacy, store.BridgeStatus.SecureBridge);
    }

    [Theory]
    [InlineData("https://dmxmoney-remote-relay.qm7ws5twn7.workers.dev/relay/desktop", "https://dmxmoney-remote-relay.qm7ws5twn7.workers.dev/mobile", true)]
    [InlineData("https://dmxmoney-remote-relay.qm7ws5twn7.workers.dev:443/relay/desktop", "https://dmxmoney-remote-relay.qm7ws5twn7.workers.dev/mobile", true)]
    [InlineData("https://dmxmoney-remote-relay.qm7ws5twn7.workers.dev/relay/desktop", "https://dmxmoney-companion.pages.dev/", false)]
    [InlineData("https://dmxmoney-remote-relay.qm7ws5twn7.workers.dev/relay/desktop", "http://dmxmoney-companion.pages.dev/", true)]
    [InlineData("https://companion.example.com/relay/desktop", "https://companion.example.com/mobile", false)]
    [InlineData("https://dmxmoney-remote-relay.qm7ws5twn7.workers.dev.other.example/relay/desktop", "https://companion.example.com/mobile", false)]
    [InlineData("https://user@dmxmoney-remote-relay.qm7ws5twn7.workers.dev/relay/desktop", "https://companion.example.com/mobile", false)]
    [InlineData("https://dmxmoney-remote-relay.qm7ws5twn7.workers.dev:444/relay/desktop", "https://companion.example.com/mobile", false)]
    public void OnlyTheOfficialRelayRequiresThePagesMigration(string api, string app, bool required)
    {
        using var store = new EngineStore(DmxEngine.OpenInMemory());
        var remote = Remote(true) with { ApiUrl = api, AppUrl = app };
        store.BridgeStatus = new CompanionStatus(true, true, null, null, null, 1, remote);
        using var settings = new SettingsViewModel(store);

        Assert.Equal(required, settings.NeedsHostedPwaUpdate);
        Assert.Equal(required, settings.CompanionMigrationRequired);
        if (required)
        {
            Assert.Equal("Mettre à jour le compagnon", settings.CompanionMigrationLabel);
            settings.MigrateBridgeCommand.Execute(null);
            Assert.NotNull(store.Confirmation);
            Assert.Equal("Mettre à jour le compagnon ?", store.Confirmation.Title);
            Assert.Contains("saisies en attente", store.Confirmation.Message);
            Assert.Same(remote, store.BridgeStatus.SecureBridge);
            Assert.False(settings.BridgeBusy);
        }
    }
}
