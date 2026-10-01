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
        var hash = Convert.ToBase64String(new byte[32]);
        Assert.True(UpdatePolicy.IsValidPackage("DmxMoney-2.0.7-win-x64-full.nupkg", hash, 100));
        Assert.False(UpdatePolicy.IsValidPackage("DmxMoney.nupkg", null, 100));
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
        => Assert.False(UpdatePolicy.IsValidPackage(fileName, Convert.ToBase64String(new byte[32]), 100));
}
