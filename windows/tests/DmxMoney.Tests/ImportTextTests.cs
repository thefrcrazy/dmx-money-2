using System.Text;
using DmxMoney.ViewModels;

namespace DmxMoney.Tests;

public class ImportTextTests
{
    [Fact]
    public async Task Utf8BomAndWindows1252BankCharactersArePreserved()
    {
        using var utf8 = new MemoryStream(Encoding.UTF8.GetBytes("\uFEFFCœur €"));
        Assert.Equal("Cœur €", await ImportText.ReadBoundedAsync(utf8, 1));
        Assert.Equal("€ “Cœur” – é", ImportText.Decode([0x80, 0x20, 0x93, 0x43, 0x9c, 0x75, 0x72, 0x94, 0x20, 0x96, 0x20, 0xe9]));
    }

    [Fact]
    public async Task OversizedSeekableFileIsRejectedWithoutReadingIt()
    {
        using var stream = new MemoryStream(new byte[1024 * 1024 + 1]);
        var error = await Assert.ThrowsAsync<IOException>(() => ImportText.ReadBoundedAsync(stream, 1));
        Assert.Contains("1 Mio", error.Message);
        Assert.Equal(0, stream.Position);
    }

    [Fact]
    public async Task NonSeekableOrGrowingFileCannotBypassTheLimit()
    {
        using var stream = new NonSeekableStream(new byte[1024 * 1024 + 1]);
        await Assert.ThrowsAsync<IOException>(() => ImportText.ReadBoundedAsync(stream, 1));
        Assert.True(stream.CanRead); // The picker owns and disposes the stream.
    }

    private sealed class NonSeekableStream(byte[] bytes) : MemoryStream(bytes)
    {
        public override bool CanSeek => false;
        public override long Length => throw new NotSupportedException();
    }
}
