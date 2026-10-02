using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Security;
using System.Security.Cryptography;
using System.Security.Cryptography.X509Certificates;
using System.Text.RegularExpressions;

namespace DmxMoney;

public static class SelfSignedPfx
{
    private const uint UserKeySet = 0x1000;
    private const uint AlwaysCng = 0x200;
    private const uint NoPersistKey = 0x8000;

    [StructLayout(LayoutKind.Sequential)]
    private struct Blob { public uint Length; public IntPtr Data; }

    [DllImport("crypt32.dll", SetLastError = true)]
    private static extern IntPtr PFXImportCertStore(ref Blob data, IntPtr password, uint flags);
    [DllImport("crypt32.dll", SetLastError = true)]
    private static extern IntPtr CertEnumCertificatesInStore(IntPtr store, IntPtr previous);
    [DllImport("crypt32.dll")]
    private static extern bool CertFreeCertificateContext(IntPtr context);
    [DllImport("crypt32.dll")]
    private static extern bool CertCloseStore(IntPtr store, uint flags);

    // Password never becomes a managed string; only CurrentUser/My receives the certificate.
    public static string Import(byte[] pfx, SecureString password, string expectedThumbprint)
    {
        if (!OperatingSystem.IsWindows()) throw new PlatformNotSupportedException("L'import PFX nécessite Windows.");
        if (pfx == null || pfx.Length == 0 || pfx.Length > 4 * 1024 * 1024)
            throw new ArgumentException("PFX vide ou trop volumineux.");
        if (password == null || password.Length == 0) throw new ArgumentException("Un mot de passe PFX non vide est requis.");
        if (expectedThumbprint == null || !Regex.IsMatch(expectedThumbprint, @"\A[0-9a-fA-F]{40}\z"))
            throw new ArgumentException("ExpectedThumbprint invalide : 40 caractères hexadécimaux requis.");

        using var personal = new X509Store(StoreName.My, StoreLocation.CurrentUser);
        personal.Open(OpenFlags.ReadWrite);
        foreach (var existing in personal.Certificates)
        {
            using (existing)
                if (StringComparer.OrdinalIgnoreCase.Equals(existing.Thumbprint, expectedThumbprint))
                    throw new InvalidOperationException("Le certificat existe déjà dans CurrentUser/My ; aucune clé existante ne sera remplacée.");
        }

        var pinned = GCHandle.Alloc(pfx, GCHandleType.Pinned);
        IntPtr nativePassword = IntPtr.Zero;
        X509Certificate2 imported = null;
        bool added = false;
        try
        {
            nativePassword = Marshal.SecureStringToGlobalAllocUnicode(password);
            var blob = new Blob { Length = checked((uint)pfx.Length), Data = pinned.AddrOfPinnedObject() };
            // Refuse any extra certificate/key before persisting anything.
            using (var preview = ReadSingle(ref blob, nativePassword, UserKeySet | AlwaysCng | NoPersistKey))
                Validate(preview, expectedThumbprint);

            // CRYPT_EXPORTABLE and PKCS12_ALLOW_OVERWRITE_KEY are deliberately absent.
            imported = ReadSingle(ref blob, nativePassword, UserKeySet | AlwaysCng);
            Validate(imported, expectedThumbprint);
            using (var key = imported.GetRSAPrivateKey())
            {
                if (key is not RSACng cng || cng.Key.ExportPolicy != CngExportPolicies.None)
                    throw new CryptographicException("Le PFX doit fournir une clé CNG non exportable.");
            }
            personal.Add(imported);
            added = true;
            return imported.Thumbprint;
        }
        catch
        {
            if (imported != null)
            {
                if (added) personal.Remove(imported);
                using var key = imported.GetRSAPrivateKey();
                if (key is RSACng cng) cng.Key.Delete();
            }
            throw;
        }
        finally
        {
            imported?.Dispose();
            if (nativePassword != IntPtr.Zero) Marshal.ZeroFreeGlobalAllocUnicode(nativePassword);
            pinned.Free();
        }
    }

    private static X509Certificate2 ReadSingle(ref Blob blob, IntPtr password, uint flags)
    {
        IntPtr store = PFXImportCertStore(ref blob, password, flags);
        if (store == IntPtr.Zero)
        {
            int error = Marshal.GetLastWin32Error();
            throw new Win32Exception(error, $"Import PFX refusé (0x{error:X8}) : {new Win32Exception(error).Message}");
        }
        IntPtr context = IntPtr.Zero;
        X509Certificate2 certificate = null;
        try
        {
            context = CertEnumCertificatesInStore(store, IntPtr.Zero);
            if (context == IntPtr.Zero) throw new CryptographicException("Le PFX doit contenir exactement un certificat et sa clé.");
#pragma warning disable SYSLIB0057
            certificate = new X509Certificate2(context);
#pragma warning restore SYSLIB0057
            // Enumeration frees the previous context; the X509Certificate2 owns its duplicate.
            context = CertEnumCertificatesInStore(store, context);
            if (context != IntPtr.Zero) throw new CryptographicException("Le PFX doit contenir exactement un certificat et sa clé.");
            var result = certificate;
            certificate = null;
            return result;
        }
        finally
        {
            certificate?.Dispose();
            if (context != IntPtr.Zero) CertFreeCertificateContext(context);
            CertCloseStore(store, 0);
        }
    }

    private static void Validate(X509Certificate2 certificate, string expectedThumbprint)
    {
        if (!StringComparer.OrdinalIgnoreCase.Equals(certificate.Thumbprint, expectedThumbprint))
            throw new CryptographicException("Le certificat PFX ne correspond pas à ExpectedThumbprint.");
        if (!certificate.HasPrivateKey || certificate.Subject != certificate.Issuer ||
            DateTime.UtcNow < certificate.NotBefore.ToUniversalTime() || DateTime.UtcNow >= certificate.NotAfter.ToUniversalTime())
            throw new CryptographicException("Le PFX exige un certificat autosigné valide et sa clé privée.");
        bool codeSigning = false;
        foreach (var extension in certificate.Extensions)
            if (extension is X509EnhancedKeyUsageExtension eku)
                foreach (var oid in eku.EnhancedKeyUsages)
                    if (oid.Value == "1.3.6.1.5.5.7.3.3") codeSigning = true;
        using var rsa = certificate.GetRSAPublicKey();
        if (!codeSigning || rsa == null || rsa.KeySize < 3072)
            throw new CryptographicException("Le PFX exige RSA 3072 bits minimum et l'usage Code Signing.");
    }
}
