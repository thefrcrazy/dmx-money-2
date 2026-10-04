using System.Text;

namespace DmxMoney.ViewModels;

/// <summary>Lecture et décodage bornés hors du thread d'interface ; le flux reste à l'appelant.</summary>
public static class ImportText
{
    private static readonly Encoding Utf8 = new UTF8Encoding(false, true);
    private static readonly Encoding Windows1252 = CodePagesEncodingProvider.Instance.GetEncoding(1252)!;

    public static Task<string> ReadBoundedAsync(Stream stream, int maxMiB)
    {
        ArgumentOutOfRangeException.ThrowIfLessThan(maxMiB, 1);
        ArgumentOutOfRangeException.ThrowIfGreaterThan(maxMiB, 64);
        var maxBytes = (long)maxMiB * 1024 * 1024;
        return Task.Run(async () =>
        {
            IOException TooLarge() => new($"Le fichier dépasse la limite de {maxMiB} Mio.");
            var length = stream.CanSeek ? stream.Length : 0;
            if (length > maxBytes) throw TooLarge();
            using var memory = new MemoryStream((int)length);
            var buffer = new byte[64 * 1024];
            int read;
            while ((read = await stream.ReadAsync(buffer).ConfigureAwait(false)) > 0)
            {
                if (memory.Length + read > maxBytes) throw TooLarge();
                memory.Write(buffer, 0, read);
            }
            return Decode(memory.GetBuffer().AsSpan(0, checked((int)memory.Length)));
        });
    }

    public static string Decode(ReadOnlySpan<byte> bytes)
    {
        string text;
        try { text = Utf8.GetString(bytes); }
        catch (DecoderFallbackException) { text = Windows1252.GetString(bytes); }
        return text.StartsWith('\uFEFF') ? text[1..] : text;
    }
}
