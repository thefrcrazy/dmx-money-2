using System.Text;
using DmxMoney.ViewModels;
using Microsoft.UI.Xaml;
using Velopack;
using Velopack.Sources;
using Windows.ApplicationModel.DataTransfer;
using Windows.Storage;
using Windows.Storage.Pickers;
using WinRT.Interop;

namespace DmxMoney.App;

/// <summary>Sélecteurs de fichiers, presse-papiers et mises à jour Velopack.</summary>
public sealed class WindowsPlatformServices : IPlatformServices
{
    private const string DefaultUpdateUrl = "https://github.com/TheFRcRaZy/dmx-money-2/releases/latest/download";

    private readonly Window window;
    private readonly UpdateManager? manager;
    private UpdateInfo? pending;
    private bool isUpdating;

    public WindowsPlatformServices(Window window)
    {
        this.window = window;
        Encoding.RegisterProvider(CodePagesEncodingProvider.Instance);
        var url = Environment.GetEnvironmentVariable("DMXMONEY_UPDATE_URL");
        try
        {
            if (!string.IsNullOrWhiteSpace(url))
            {
                manager = new UpdateManager(new SimpleWebSource(url));
            }
            else
            {
                manager = new UpdateManager(new GithubSource("https://github.com/TheFRcRaZy/dmx-money-2", null, prerelease: true));
            }
        }
        catch (Exception)
        {
            manager = null;
        }
    }

    public bool UpdateAvailable => pending is not null;

    public bool IncludePrereleases
    {
        get
        {
            try
            {
                var raw = ApplicationData.Current.LocalSettings.Values["DmxIncludePrereleases"];
                return raw is bool b ? b : true;
            }
            catch (Exception)
            {
                return true;
            }
        }
        set
        {
            try
            {
                ApplicationData.Current.LocalSettings.Values["DmxIncludePrereleases"] = value;
            }
            catch (Exception)
            {
            }
        }
    }

    public async Task ExportBackupAsync(string content, string suggestedFileName)
    {
        var picker = new FileSavePicker
        {
            SuggestedStartLocation = PickerLocationId.DocumentsLibrary,
            SuggestedFileName = suggestedFileName,
        };
        picker.FileTypeChoices.Add("Sauvegarde DmxMoney", [".dmx"]);
        InitializeWithWindow.Initialize(picker, WindowNative.GetWindowHandle(window));
        var file = await picker.PickSaveFileAsync();
        if (file is null)
        {
            return;
        }
        await FileIO.WriteTextAsync(file, content);
    }

    public async Task<(string Content, string FileName)?> PickImportFileAsync()
    {
        var picker = new FileOpenPicker { SuggestedStartLocation = PickerLocationId.DocumentsLibrary };
        foreach (var extension in new[] { ".dmx", ".json", ".csv", ".qif", ".ofx", ".txt" })
        {
            picker.FileTypeFilter.Add(extension);
        }
        InitializeWithWindow.Initialize(picker, WindowNative.GetWindowHandle(window));
        var file = await picker.PickSingleFileAsync();
        if (file is null)
        {
            return null;
        }
        await using var stream = await file.OpenStreamForReadAsync();
        using var memory = new MemoryStream();
        await stream.CopyToAsync(memory);
        return (DecodeText(memory.ToArray()), file.Name);
    }

    /// <summary>Les relevés bancaires sont souvent encodés en Windows-1252.</summary>
    public static string DecodeText(byte[] bytes)
    {
        foreach (var encoding in new[] { new UTF8Encoding(false, true), (Encoding)Encoding.GetEncoding(1252), Encoding.Latin1 })
        {
            try
            {
                var text = encoding.GetString(bytes);
                return text.StartsWith('﻿') ? text[1..] : text;
            }
            catch (DecoderFallbackException)
            {
            }
        }
        return string.Empty;
    }

    public void CopyToClipboard(string text)
    {
        var package = new DataPackage();
        package.SetText(text);
        Clipboard.SetContent(package);
    }

    public async Task CheckForUpdatesAsync()
    {
        if (isUpdating) return;
        isUpdating = true;
        try
        {
            await InstallAvailableUpdateAsync();
        }
        catch (TimeoutException)
        {
            pending = null;
            App.Store.ShowToast("La recherche de mise à jour a expiré. Réessayez.");
        }
        catch (OperationCanceledException)
        {
            pending = null;
            App.Store.ShowToast("Le téléchargement de la mise à jour a expiré. Réessayez.");
        }
        catch (Exception)
        {
            pending = null;
            App.Store.ShowToast("Impossible d'installer la mise à jour. Réessayez ou téléchargez l'installateur manuellement.");
        }
        finally
        {
            isUpdating = false;
        }
    }

    private async Task InstallAvailableUpdateAsync()
    {
        var url = Environment.GetEnvironmentVariable("DMXMONEY_UPDATE_URL");
        var activeManager = manager;
        if (string.IsNullOrWhiteSpace(url))
        {
            try
            {
                activeManager = new UpdateManager(new GithubSource("https://github.com/TheFRcRaZy/dmx-money-2", null, prerelease: IncludePrereleases));
            }
            catch (Exception)
            {
                activeManager = manager;
            }
        }

        if (activeManager is null || !activeManager.IsInstalled)
        {
            App.Store.ShowToast("Mise à jour automatique disponible sur la version installée.");
            return;
        }
        pending = await activeManager.CheckForUpdatesAsync().WaitAsync(TimeSpan.FromSeconds(30));
        if (pending is null)
        {
            App.Store.ShowToast("L'application est à jour.");
            return;
        }
        App.Store.ShowToast("Téléchargement de la mise à jour…");
        using var timeout = new CancellationTokenSource(TimeSpan.FromMinutes(10));
        await activeManager.DownloadUpdatesAsync(pending, null, timeout.Token);
        activeManager.ApplyUpdatesAndRestart(pending);
    }
}

/// <summary>Taille et position de la fenêtre, conservées hors du paquet d'installation.</summary>
public static class WindowState
{
    private sealed record Frame(int Width, int Height, int X, int Y);

    private static string Path => System.IO.Path.Combine(EngineStore.DataDirectory(), "window.json");

    public static (int Width, int Height, int X, int Y)? Load()
    {
        try
        {
            if (!File.Exists(Path))
            {
                return null;
            }
            var frame = System.Text.Json.JsonSerializer.Deserialize<Frame>(File.ReadAllText(Path));
            return frame is null || frame.Width < 900 || frame.Height < 600 ? null : (frame.Width, frame.Height, frame.X, frame.Y);
        }
        catch (Exception)
        {
            return null;
        }
    }

    public static void Save(int width, int height, int x, int y)
    {
        try
        {
            Directory.CreateDirectory(EngineStore.DataDirectory());
            File.WriteAllText(Path, System.Text.Json.JsonSerializer.Serialize(new Frame(width, height, x, y)));
        }
        catch (Exception)
        {
            // La position de la fenêtre n'est pas critique.
        }
    }
}
