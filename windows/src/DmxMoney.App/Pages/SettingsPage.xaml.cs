using System.ComponentModel;
using DmxMoney.Interop;
using DmxMoney.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;
using Microsoft.UI.Xaml.Shapes;

namespace DmxMoney.App;

public sealed partial class SettingsPage : Page
{
    public SettingsViewModel ViewModel { get; private set; } = null!;

    private bool syncing;
    private string? shownPairingUrl;

    public SettingsPage()
    {
        InitializeComponent();
        NavigationCacheMode = NavigationCacheMode.Required;
    }

    protected override void OnNavigatedTo(NavigationEventArgs args)
    {
        var shell = (ShellViewModel)args.Parameter;
        if (!ReferenceEquals(ViewModel, shell.SettingsPage))
        {
            if (ViewModel is not null)
            {
                ViewModel.PropertyChanged -= OnViewModelChanged;
            }
            ViewModel = shell.SettingsPage;
            ViewModel.PropertyChanged += OnViewModelChanged;
            BuildAccentSwatches();
        }
        Bindings.Update();
        ViewModel.Store.RefreshBridgeStatus();
        UpdateVisuals();
    }

    private void OnViewModelChanged(object? sender, PropertyChangedEventArgs args) => UpdateVisuals();

    private void BuildAccentSwatches()
    {
        var swatches = ViewModel.AccentColors.Select(hex =>
        {
            var button = new Button
            {
                Width = 28,
                Height = 28,
                CornerRadius = new CornerRadius(14),
                Padding = new Thickness(0),
                Background = Format.Brush(hex),
                BorderThickness = new Thickness(0),
                Tag = hex,
            };
            button.Click += OnAccentClicked;
            return button;
        }).ToList();
        AccentColors.ItemsSource = swatches;
    }

    private void UpdateVisuals()
    {
        syncing = true;
        LightTheme.IsChecked = ViewModel.Theme == Theme.Light;
        DarkTheme.IsChecked = ViewModel.Theme == Theme.Dark;
        SystemTheme.IsChecked = ViewModel.Theme == Theme.System;
        DefaultAccent.BorderThickness = new Thickness(ViewModel.IsDefaultAccent ? 2 : 1);

        BridgeSection.Visibility = ViewModel.BridgeAvailable ? Visibility.Visible : Visibility.Collapsed;
        BridgeSwitch.IsOn = ViewModel.BridgeSwitchOn;
        BridgeSwitch.IsEnabled = !ViewModel.BridgeBusy;
        BridgeBadge.Text = ViewModel.BridgeStateLabel;
        BridgeDetail.Text = ViewModel.BridgeStateDetail;
        AppUrl.Text = ViewModel.SecureBridge?.AppUrl ?? "Provisionnement automatique en attente";
        ApiUrl.Text = ViewModel.SecureBridge?.ApiUrl ?? "Non active";

        BridgeSteps.ItemsSource = ViewModel.BridgeSteps.Select(step => new BridgeStepRow(step.Label, step.Value, step.Ready, step.Icon)).ToList();
        BridgeError.Text = ViewModel.SecureBridge?.LastError ?? string.Empty;
        BridgeError.Visibility = string.IsNullOrEmpty(ViewModel.SecureBridge?.LastError) ? Visibility.Collapsed : Visibility.Visible;

        PairingLabel.Text = ViewModel.PairingButtonLabel;
        PairingButton.IsEnabled = ViewModel.SecureBridge?.Active == true && !ViewModel.BridgeBusy;
        CopyButton.Visibility = ViewModel.SecureBridge?.PairingUrl is null ? Visibility.Collapsed : Visibility.Visible;
        UpdateQr(ViewModel.SecureBridge?.PairingUrl);

        var passkeys = ViewModel.Passkeys;
        PasskeysTitle.Text = $"MOBILES APPAIRÉS ({passkeys.Count})";
        Passkeys.ItemsSource = passkeys;
        NoPasskeys.Visibility = passkeys.Count == 0 ? Visibility.Visible : Visibility.Collapsed;

        VersionText.Text = $"Version {ViewModel.Version}";
        UpdateStatus.Text = ViewModel.UpdateAvailable ? "Nouvelle version disponible" : "L'application est à jour";
        UpdateLabel.Text = ViewModel.UpdateAvailable ? "Installer" : "Vérifier";
        syncing = false;
    }

    private async void UpdateQr(string? url)
    {
        if (url == shownPairingUrl)
        {
            return;
        }
        shownPairingUrl = url;
        if (url is null)
        {
            QrCode.Source = null;
            QrCode.Visibility = Visibility.Collapsed;
            QrEmpty.Visibility = Visibility.Visible;
            QrEmpty.Text = ViewModel.QrEmptyMessage;
            return;
        }
        var image = await QrImage.CreateAsync(url);
        QrCode.Source = image;
        QrCode.Visibility = image is null ? Visibility.Collapsed : Visibility.Visible;
        QrEmpty.Visibility = image is null ? Visibility.Visible : Visibility.Collapsed;
    }

    private void OnThemeClicked(object sender, RoutedEventArgs args)
    {
        if (syncing || sender is not FrameworkElement { Tag: string tag })
        {
            return;
        }
        ViewModel.Theme = tag switch
        {
            "light" => Theme.Light,
            "dark" => Theme.Dark,
            _ => Theme.System,
        };
        UpdateVisuals();
    }

    private void OnAccentClicked(object sender, RoutedEventArgs args)
    {
        if (sender is FrameworkElement { Tag: string hex })
        {
            ViewModel.SetAccentCommand.Execute(hex);
        }
    }

    private void OnDefaultAccent(object sender, RoutedEventArgs args) => ViewModel.UseDefaultAccentCommand.Execute(null);

    private void OnBridgeToggled(object sender, RoutedEventArgs args)
    {
        if (!syncing && BridgeSwitch.IsOn != ViewModel.BridgeSwitchOn)
        {
            ViewModel.SetBridgeEnabledCommand.Execute(BridgeSwitch.IsOn);
        }
    }

    private void OnRegeneratePairing(object sender, RoutedEventArgs args) => ViewModel.RegeneratePairingCommand.Execute(null);

    private void OnRevokePasskey(object sender, RoutedEventArgs args)
    {
        if (sender is FrameworkElement { Tag: PasskeyInfo passkey })
        {
            ViewModel.RevokePasskeyCommand.Execute(passkey);
        }
    }
}

/// <summary>Étape de préparation du pont (PWA, provisionnement, DNS, certificat, API).</summary>
public sealed class BridgeStepRow : UserControl
{
    public BridgeStepRow(string label, string value, bool ready, string icon)
    {
        // Border est scellé en WinUI 3 : la ligne contient sa bordure au lieu d'en hériter.
        var row = new Border
        {
            CornerRadius = new CornerRadius(10),
            Padding = new Thickness(10, 7, 10, 7),
            Background = (Microsoft.UI.Xaml.Media.Brush)Application.Current.Resources["DmxSubtleBackground"],
        };
        Margin = new Thickness(0, 0, 0, 6);

        var grid = new Grid { ColumnSpacing = 10 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });

        var badge = new Border
        {
            Width = 24,
            Height = 24,
            CornerRadius = new CornerRadius(6),
            Background = ready ? Format.Tint("#10b981") : Format.Tint("#9ca3af"),
            Child = new LucideIcon
            {
                Glyph = ready ? "CheckCircle2" : icon,
                Size = 13,
                Stroke = ready ? Palette.Income : Palette.Resource("TextFillColorSecondaryBrush"),
                HorizontalAlignment = HorizontalAlignment.Center,
                VerticalAlignment = VerticalAlignment.Center,
            },
        };
        Grid.SetColumn(badge, 0);
        grid.Children.Add(badge);

        var stack = new StackPanel();
        stack.Children.Add(new TextBlock { Text = label, FontSize = 12, FontWeight = Microsoft.UI.Text.FontWeights.Medium });
        stack.Children.Add(new TextBlock
        {
            Text = value,
            FontSize = 11,
            Foreground = Palette.Resource("TextFillColorSecondaryBrush"),
            TextTrimming = TextTrimming.CharacterEllipsis,
        });
        Grid.SetColumn(stack, 1);
        grid.Children.Add(stack);
        row.Child = grid;
        Content = row;
    }
}
