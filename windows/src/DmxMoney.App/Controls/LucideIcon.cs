using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Markup;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;
// Les using implicites apportent aussi System.IO.Path.
using Path = Microsoft.UI.Xaml.Shapes.Path;

namespace DmxMoney.App;

/// <summary>
/// Icône Lucide dessinée au trait : elle suit la couleur du texte (ou une couleur donnée),
/// contrairement à une image SVG.
/// </summary>
public sealed class LucideIcon : Path
{
    public static readonly DependencyProperty GlyphProperty = DependencyProperty.Register(
        nameof(Glyph),
        typeof(string),
        typeof(LucideIcon),
        new PropertyMetadata("Tag", (sender, _) => ((LucideIcon)sender).Apply()));

    public static readonly DependencyProperty SizeProperty = DependencyProperty.Register(
        nameof(Size),
        typeof(double),
        typeof(LucideIcon),
        new PropertyMetadata(16.0, (sender, _) => ((LucideIcon)sender).ApplySize()));

    public string Glyph
    {
        get => (string)GetValue(GlyphProperty);
        set => SetValue(GlyphProperty, value);
    }

    public double Size
    {
        get => (double)GetValue(SizeProperty);
        set => SetValue(SizeProperty, value);
    }

    public LucideIcon()
    {
        Stretch = Stretch.Uniform;
        StrokeLineJoin = PenLineJoin.Round;
        StrokeStartLineCap = PenLineCap.Round;
        StrokeEndLineCap = PenLineCap.Round;
        Fill = null;
        Stroke = (Brush)Application.Current.Resources["TextFillColorPrimaryBrush"];
        ApplySize();
        Apply();
    }

    private void ApplySize()
    {
        Width = Size;
        Height = Size;
        // Le tracé Lucide est dessiné dans une boîte de 24 : on garde une épaisseur constante à l'écran.
        StrokeThickness = 2.0 * 24.0 / Math.Max(Size, 1.0);
    }

    private void Apply()
    {
        try
        {
            Data = (Geometry)XamlBindingHelper.ConvertValue(typeof(Geometry), LucideIcons.Geometry(Glyph));
        }
        catch (Exception)
        {
            Data = null;
        }
    }
}
