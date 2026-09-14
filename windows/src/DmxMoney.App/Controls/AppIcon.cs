using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace DmxMoney.App;

/// <summary>
/// Icône native de Windows : glyphe de la police système Segoe Fluent Icons (Segoe MDL2 Assets
/// sous Windows 10), choisi d'après le nom d'icône stocké en base et partagé avec les autres
/// plateformes (shared/icons/native.json).
/// </summary>
public sealed class AppIcon : FontIcon
{
    private static readonly FontFamily SymbolFont = new("Segoe Fluent Icons, Segoe MDL2 Assets");

    public static readonly DependencyProperty IconProperty = DependencyProperty.Register(
        nameof(Icon),
        typeof(string),
        typeof(AppIcon),
        new PropertyMetadata("Tag", (sender, _) => ((AppIcon)sender).Apply()));

    public string Icon
    {
        get => (string)GetValue(IconProperty);
        set => SetValue(IconProperty, value);
    }

    public AppIcon()
    {
        FontFamily = SymbolFont;
        Apply();
    }

    private void Apply() => Glyph = FluentIcons.Glyph(Icon);
}
