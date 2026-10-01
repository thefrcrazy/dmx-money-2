using System.ComponentModel;
using System.Runtime.InteropServices;
using DmxMoney.ViewModels;

namespace DmxMoney.Tests;

public sealed class UpdatePolicyTests
{
    [Fact]
    public void DefaultRepositoryIsTheMaintainedV2()
        => Assert.Equal("https://github.com/thefrcrazy/dmx-money-2", UpdatePolicy.RepositoryUrl);

    [Theory]
    [InlineData(null)]
    [InlineData("")]
    [InlineData(" ")]
    public void EmptyOverrideUsesTheDefaultRepository(string? value)
        => Assert.Null(UpdatePolicy.ValidateSourceOverride(value));

    [Fact]
    public void HttpsOverrideIsNormalized()
        => Assert.Equal("https://updates.example.com/releases/", UpdatePolicy.ValidateSourceOverride(" https://updates.example.com/releases/ "));

    [Theory]
    [InlineData("http://updates.example.com/releases")]
    [InlineData("file:///C:/updates")]
    [InlineData("C:\\updates")]
    [InlineData("https://user:secret@updates.example.com")]
    [InlineData("https://updates.example.com/releases?token=secret")]
    [InlineData("https://updates.example.com/releases#fragment")]
    public void UnsafeOverrideIsRejected(string value)
        => Assert.Throws<ArgumentException>(() => UpdatePolicy.ValidateSourceOverride(value));

    [Fact]
    public void PackageRequiresCompleteSha256AndPositiveSize()
    {
        const string hash = "E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855";
        Assert.True(UpdatePolicy.IsValidPackage("DmxMoney-2.0.7-win-x64-full.nupkg", hash, 100));
        Assert.True(UpdatePolicy.IsValidPackage("DmxMoney-2.0.7-win-x64-full.nupkg", hash.ToLowerInvariant(), 100));
        Assert.False(UpdatePolicy.IsValidPackage("DmxMoney.nupkg", null, 100));
        Assert.False(UpdatePolicy.IsValidPackage("DmxMoney.nupkg", hash[..63], 100));
        Assert.False(UpdatePolicy.IsValidPackage("DmxMoney.nupkg", hash + "0", 100));
        Assert.False(UpdatePolicy.IsValidPackage("DmxMoney.nupkg", "Z" + hash[1..], 100));
        Assert.False(UpdatePolicy.IsValidPackage("DmxMoney.nupkg", "Ａ" + hash[1..], 100));
        Assert.False(UpdatePolicy.IsValidPackage("DmxMoney.nupkg", Convert.ToBase64String(new byte[32]), 100));
        Assert.False(UpdatePolicy.IsValidPackage("DmxMoney.nupkg", Convert.ToBase64String(new byte[20]), 100));
        Assert.False(UpdatePolicy.IsValidPackage("DmxMoney.nupkg", "invalid", 100));
        Assert.False(UpdatePolicy.IsValidPackage("DmxMoney.nupkg", hash, 0));
    }

    [Theory]
    [InlineData("../DmxMoney.nupkg")]
    [InlineData("..\\DmxMoney.nupkg")]
    [InlineData("C:DmxMoney.nupkg")]
    [InlineData("DmxMoney.exe")]
    [InlineData("")]
    public void PackagePathCannotEscapeTheUpdateDirectory(string fileName)
        => Assert.False(UpdatePolicy.IsValidPackage(fileName, new string('A', 64), 100));

    [Theory]
    [InlineData(577)]
    [InlineData(1260)]
    [InlineData(4551)]
    public void WindowsExecutionPolicyErrorsAreRecognizedEvenWhenWrapped(int code)
    {
        Assert.True(UpdatePolicy.IsWindowsExecutionBlocked(new Win32Exception(code)));
        Assert.True(UpdatePolicy.IsWindowsExecutionBlocked(new Exception("Échec du redémarrage", new Win32Exception(code))));
        Assert.True(UpdatePolicy.IsWindowsExecutionBlocked(new COMException("Windows", unchecked((int)(0x80070000u | (uint)code)))));
    }

    [Theory]
    [InlineData(5)] // Accès refusé : droits de fichiers, pas nécessairement contrôle d'application.
    [InlineData(2)] // Fichier absent.
    [InlineData(1223)] // Demande annulée par l'utilisateur.
    public void OrdinaryInstallationFailuresDoNotClaimASecurityPolicyBlock(int code)
        => Assert.False(UpdatePolicy.IsWindowsExecutionBlocked(new Win32Exception(code)));

    [Fact]
    public void UntrustedExceptionTextDoesNotDetermineTheDiagnosis()
        => Assert.False(UpdatePolicy.IsWindowsExecutionBlocked(new Exception("ERROR_INVALID_IMAGE_HASH 577")));
}
