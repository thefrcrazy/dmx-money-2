using DmxMoney.ViewModels;
using Velopack;
using Velopack.Sources;

namespace DmxMoney.App;

internal sealed class VerifiedUpdateManager(IUpdateSource source)
    : UpdateManager(source, new UpdateOptions { MaximumDeltasBeforeFallback = -1 })
{
    public async Task DownloadVerifiedAndRestartAsync(UpdateInfo update, CancellationToken cancellationToken)
    {
        var release = update.TargetFullRelease;
        if (!UpdatePolicy.IsValidPackage(release.FileName, release.SHA256, release.Size) ||
            string.IsNullOrEmpty(Locator.PackagesDir))
            throw new InvalidDataException("Paquet de mise à jour invalide.");
        // Le full reconstruit par les deltas n'est pas forcément identique au ZIP du flux.
        // Le téléchargement complet permet de contrôler son empreinte avant installation.
        await DownloadUpdatesAsync(update, null, cancellationToken);
        var path = Path.Combine(Locator.PackagesDir, release.FileName);
        FileStream verified;
        try
        {
            verified = await UpdatePolicy.OpenVerifiedPackageAsync(path, release.SHA256, release.Size, cancellationToken);
        }
        catch (InvalidDataException)
        {
            // Velopack réutilise un paquet déjà présent sans contrôler son empreinte.
            // Retirer uniquement ce cache permet un nouveau téléchargement à la prochaine tentative.
            try { File.Delete(path); }
            catch (IOException) { }
            catch (UnauthorizedAccessException) { }
            throw;
        }
        // Update.exe attend la sortie du parent ; le verrou empêche l'altération entre contrôle et lancement.
        await using (verified)
        {
            cancellationToken.ThrowIfCancellationRequested();
            ApplyUpdatesAndRestart(update);
        }
    }
}
