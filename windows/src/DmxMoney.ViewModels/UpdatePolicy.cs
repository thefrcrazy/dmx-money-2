using System.ComponentModel;
using System.Security.Cryptography;

namespace DmxMoney.ViewModels;

/// <summary>Règles partagées par le service de mise à jour et ses tests hors de Windows.</summary>
public static class UpdatePolicy
{
    public const string RepositoryUrl = "https://github.com/thefrcrazy/dmx-money-2";

    public static string? ValidateSourceOverride(string? value)
    {
        if (string.IsNullOrWhiteSpace(value)) return null;
        if (!Uri.TryCreate(value.Trim(), UriKind.Absolute, out var uri) ||
            uri.Scheme != Uri.UriSchemeHttps || string.IsNullOrEmpty(uri.Host) ||
            !string.IsNullOrEmpty(uri.UserInfo) || !string.IsNullOrEmpty(uri.Query) || !string.IsNullOrEmpty(uri.Fragment))
        {
            throw new ArgumentException("DMXMONEY_UPDATE_URL doit être une URL HTTPS sans identifiants, paramètres ni fragment.");
        }
        return uri.AbsoluteUri;
    }

    public static bool IsValidPackage(string? fileName, string? sha256, long size)
    {
        if (string.IsNullOrEmpty(fileName) || size <= 0 ||
            fileName.IndexOfAny(['/', '\\', ':', '<', '>', '"', '|', '?', '*']) >= 0 ||
            !fileName.EndsWith(".nupkg", StringComparison.OrdinalIgnoreCase) || sha256?.Length != 64)
        {
            return false;
        }
        foreach (var character in fileName)
            if (character < 0x20) return false;
        // Velopack 0.0.1298 produit 32 octets de SHA-256 en 64 chiffres hexadécimaux.
        foreach (var digit in sha256)
            if (!char.IsAsciiHexDigit(digit)) return false;
        return true;
    }

    /// <summary>Le propriétaire garde ce flux ouvert jusqu'au lancement de l'installation.</summary>
    public static async Task<FileStream> OpenVerifiedPackageAsync(string path, string sha256, long size,
        CancellationToken cancellationToken = default)
    {
        if (!IsValidPackage(Path.GetFileName(path), sha256, size))
            throw new InvalidDataException("Métadonnées de mise à jour invalides.");
        cancellationToken.ThrowIfCancellationRequested();
        var stream = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.Read, 128 * 1024,
            FileOptions.Asynchronous | FileOptions.SequentialScan);
        try
        {
            if (stream.Length != size)
                throw new InvalidDataException("Taille du paquet de mise à jour incorrecte.");
            var actual = await SHA256.HashDataAsync(stream, cancellationToken).ConfigureAwait(false);
            if (!CryptographicOperations.FixedTimeEquals(actual, Convert.FromHexString(sha256)))
                throw new InvalidDataException("Empreinte SHA-256 du paquet de mise à jour incorrecte.");
            stream.Position = 0;
            return stream;
        }
        catch
        {
            await stream.DisposeAsync().ConfigureAwait(false);
            throw;
        }
    }

    public static bool IsWindowsExecutionBlocked(Exception error)
    {
        for (Exception? current = error; current is not null; current = current.InnerException)
        {
            // Win32 peut être propagé directement ou encapsulé dans un HRESULT_FROM_WIN32.
            var hresult = unchecked((uint)current.HResult);
            var code = current is Win32Exception native
                ? unchecked((uint)native.NativeErrorCode)
                : (hresult & 0xffff0000u) == 0x80070000u ? hresult & 0xffffu : 0u;
            // ERROR_INVALID_IMAGE_HASH, ERROR_ACCESS_DISABLED_BY_POLICY,
            // ERROR_SYSTEM_INTEGRITY_POLICY_VIOLATION (SDK Windows, winerror.h).
            if (code is 577 or 1260 or 4551) return true;
        }
        return false;
    }
}
