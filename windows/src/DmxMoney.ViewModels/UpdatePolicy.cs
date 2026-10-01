using System.ComponentModel;

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
            fileName.IndexOfAny(['/', '\\', ':']) >= 0 ||
            !fileName.EndsWith(".nupkg", StringComparison.OrdinalIgnoreCase) || string.IsNullOrEmpty(sha256))
        {
            return false;
        }
        Span<byte> hash = stackalloc byte[32];
        return Convert.TryFromBase64String(sha256, hash, out var length) && length == hash.Length;
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
