using System.Security.Cryptography;
using DmxMoney.ViewModels;
using Xunit;

namespace DmxMoney.Tests;

public sealed class UpdatePackageTests
{
    [Theory]
    [InlineData(false)]
    [InlineData(true)]
    public async Task ExistingPackageIsVerifiedAndReturnedWithReadLock(bool lowerCase)
    {
        var path = NewPackagePath();
        byte[] bytes = [1, 2, 3, 4];
        await File.WriteAllBytesAsync(path, bytes);
        try
        {
            var hash = Convert.ToHexString(SHA256.HashData(bytes));
            await using (var verified = await UpdatePolicy.OpenVerifiedPackageAsync(path,
                lowerCase ? hash.ToLowerInvariant() : hash, bytes.Length))
            {
                Assert.Equal(0, verified.Position);
                Assert.Equal(1, verified.ReadByte());
                // Le partage restrictif est imposé par Windows ; Unix a une autre sémantique de verrouillage.
                if (OperatingSystem.IsWindows())
                {
                    Assert.Throws<IOException>(() => File.Open(path, FileMode.Open, FileAccess.Write, FileShare.ReadWrite).Dispose());
                    Assert.Throws<IOException>(() => File.Delete(path));
                }
            }
            // Le verrou est libéré par le propriétaire, sans rendre la lecture du cache destructive.
            await File.WriteAllBytesAsync(path, bytes);
        }
        finally { File.Delete(path); }
    }

    [Theory]
    [InlineData(3, false)]
    [InlineData(5, false)]
    [InlineData(4, true)]
    public async Task CorruptExistingPackageCannotReachInstallation(long size, bool corruptHash)
    {
        var path = NewPackagePath();
        byte[] bytes = [1, 2, 3, 4];
        await File.WriteAllBytesAsync(path, bytes);
        try
        {
            var hash = corruptHash ? new string('0', 64) : Convert.ToHexString(SHA256.HashData(bytes));
            await Assert.ThrowsAsync<InvalidDataException>(() => UpdatePolicy.OpenVerifiedPackageAsync(path, hash, size));
            Assert.Equal(bytes, await File.ReadAllBytesAsync(path));
            // Un refus libère aussi le flux, afin que le service puisse retirer le cache invalide.
            using var writable = File.Open(path, FileMode.Open, FileAccess.Write, FileShare.None);
        }
        finally { File.Delete(path); }
    }

    [Fact]
    public async Task CancelledVerificationDoesNotOpenOrEraseTheCachedPackage()
    {
        var path = NewPackagePath();
        await File.WriteAllBytesAsync(path, [1, 2, 3, 4]);
        try
        {
            using var cancellation = new CancellationTokenSource();
            cancellation.Cancel();
            await Assert.ThrowsAnyAsync<OperationCanceledException>(() =>
                UpdatePolicy.OpenVerifiedPackageAsync(path, new string('0', 64), 4, cancellation.Token));
            Assert.True(File.Exists(path));
        }
        finally { File.Delete(path); }
    }

    private static string NewPackagePath() => Path.Combine(Path.GetTempPath(), $"DmxMoney-{Guid.NewGuid():N}.nupkg");
}
