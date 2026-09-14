using Microsoft.UI;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Shapes;
using Windows.Foundation;
using Windows.UI;
// Les using implicites apportent aussi System.IO.Path.
using Path = Microsoft.UI.Xaml.Shapes.Path;

namespace DmxMoney.App;

public sealed record ChartLineData(
    string Id,
    string Name,
    string ColorHex,
    IReadOnlyList<double> Values,
    bool Dashed = false,
    bool Filled = true,
    bool InLegend = true);

public sealed record ChartReference(double Value, string ColorHex, bool Dashed = false);

public sealed record ChartMarkerData(int Index, string ColorHex);

public sealed record ChartBarGroup(string Label, IReadOnlyList<double> Values);

public sealed record DonutSliceData(string Id, string Label, double Value, string ColorHex, bool Hidden = false);

public sealed record TooltipEntry(string Name, string ColorHex, string Value);

/// <summary>Graduations « rondes » et libellés courts des axes.</summary>
public static class ChartScale
{
    public static IReadOnlyList<double> NiceTicks(double minimum, double maximum, int count = 5)
    {
        if (double.IsNaN(minimum) || double.IsNaN(maximum))
        {
            return [0];
        }
        var low = minimum;
        var high = maximum;
        if (Math.Abs(high - low) < double.Epsilon)
        {
            low -= 1;
            high += 1;
        }
        var rawStep = (high - low) / Math.Max(count - 1, 1);
        var magnitude = Math.Pow(10, Math.Floor(Math.Log10(rawStep)));
        var residual = rawStep / magnitude;
        var step = residual > 5 ? 10 * magnitude : residual > 2 ? 5 * magnitude : residual > 1 ? 2 * magnitude : magnitude;
        var start = Math.Floor(low / step) * step;
        var end = Math.Ceiling(high / step) * step;
        var ticks = new List<double>();
        for (var value = start; value <= end + step * 0.5; value += step)
        {
            ticks.Add(value);
        }
        return ticks;
    }

    public static string CompactLabel(double value)
    {
        var absolute = Math.Abs(value);
        return absolute switch
        {
            >= 1_000_000 => (value / 1_000_000).ToString("0.#", System.Globalization.CultureInfo.GetCultureInfo("fr-FR")) + "M",
            >= 10_000 => (value / 1_000).ToString("0", System.Globalization.CultureInfo.GetCultureInfo("fr-FR")) + "k",
            >= 1_000 => (value / 1_000).ToString("0.#", System.Globalization.CultureInfo.GetCultureInfo("fr-FR")) + "k",
            _ => value.ToString("0", System.Globalization.CultureInfo.GetCultureInfo("fr-FR")),
        };
    }
}

/// <summary>Base commune : canevas redessiné à chaque changement de taille ou de données.</summary>
public abstract class ChartViewBase : ContentControl
{
    protected const double AxisWidth = 46;
    protected const double LabelHeight = 20;

    protected Canvas Surface { get; } = new();

    protected ChartViewBase()
    {
        Content = Surface;
        HorizontalContentAlignment = HorizontalAlignment.Stretch;
        VerticalContentAlignment = VerticalAlignment.Stretch;
        IsTabStop = false;
        SizeChanged += (_, _) => Render();
        ActualThemeChanged += (_, _) => Render();
    }

    protected Brush GridBrush => new SolidColorBrush(Palette.ToColor(ActualTheme == ElementTheme.Dark ? "#22FFFFFF" : "#14000000", Colors.Gray));

    protected Brush SecondaryBrush => (Brush)Application.Current.Resources["TextFillColorSecondaryBrush"];

    protected Brush PrimaryBrush => (Brush)Application.Current.Resources["TextFillColorPrimaryBrush"];

    protected Rect PlotRect => new(
        AxisWidth,
        6,
        Math.Max(ActualWidth - AxisWidth - 10, 1),
        Math.Max(ActualHeight - LabelHeight - 14, 1));

    public abstract void Render();

    protected void AddLine(double x1, double y1, double x2, double y2, Brush brush, double thickness = 1, DoubleCollection? dashes = null)
    {
        var line = new Line
        {
            X1 = x1,
            Y1 = y1,
            X2 = x2,
            Y2 = y2,
            Stroke = brush,
            StrokeThickness = thickness,
        };
        if (dashes is not null)
        {
            line.StrokeDashArray = dashes;
        }
        Surface.Children.Add(line);
    }

    protected void AddLabel(string text, double x, double y, double size = 10, Brush? brush = null, TextAlignment alignment = TextAlignment.Left, double width = 0)
    {
        var label = new TextBlock
        {
            Text = text,
            FontSize = size,
            Foreground = brush ?? SecondaryBrush,
            TextAlignment = alignment,
            TextTrimming = TextTrimming.CharacterEllipsis,
        };
        if (width > 0)
        {
            label.Width = width;
        }
        Canvas.SetLeft(label, x);
        Canvas.SetTop(label, y);
        Surface.Children.Add(label);
    }

    protected static DoubleCollection Dashes(double on, double off) => [on, off];

    /// <summary>Encadré d'inspection affiché au survol.</summary>
    protected Border BuildTooltip(string title, IEnumerable<TooltipEntry> entries, string? footer)
    {
        var stack = new StackPanel { Spacing = 4 };
        stack.Children.Add(new TextBlock { Text = title, FontSize = 12, FontWeight = Microsoft.UI.Text.FontWeights.SemiBold });
        foreach (var entry in entries)
        {
            var row = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6 };
            row.Children.Add(new Ellipse
            {
                Width = 7,
                Height = 7,
                Fill = Palette.ToBrush(entry.ColorHex, Colors.Gray),
                VerticalAlignment = VerticalAlignment.Center,
            });
            row.Children.Add(new TextBlock { Text = entry.Name, FontSize = 11 });
            row.Children.Add(new TextBlock
            {
                Text = entry.Value,
                FontSize = 11,
                FontWeight = Microsoft.UI.Text.FontWeights.SemiBold,
                Margin = new Thickness(6, 0, 0, 0),
            });
            stack.Children.Add(row);
        }
        if (!string.IsNullOrEmpty(footer))
        {
            stack.Children.Add(new TextBlock
            {
                Text = footer,
                FontSize = 10,
                Foreground = SecondaryBrush,
                TextWrapping = TextWrapping.Wrap,
                MaxWidth = 240,
            });
        }
        return new Border
        {
            Child = stack,
            Background = (Brush)Application.Current.Resources["DmxCardBackground"],
            BorderBrush = (Brush)Application.Current.Resources["DmxSeparator"],
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(8),
            Padding = new Thickness(10),
            IsHitTestVisible = false,
        };
    }
}

/// <summary>Courbes (aires, escaliers, pointillés) avec seuils, marqueurs et inspection au survol.</summary>
public sealed class LineChartView : ChartViewBase
{
    private IReadOnlyList<ChartLineData> series = [];
    private IReadOnlyList<string> labels = [];
    private IReadOnlyList<string> fullLabels = [];
    private IReadOnlyList<ChartReference> references = [];
    private IReadOnlyList<ChartMarkerData> markers = [];
    private int? hovered;

    public bool Stepped { get; set; }

    public Func<int, (IEnumerable<TooltipEntry> Entries, string? Footer)>? TooltipProvider { get; set; }

    public LineChartView()
    {
        PointerMoved += OnPointerMoved;
        PointerExited += (_, _) =>
        {
            hovered = null;
            Render();
        };
    }

    public void Update(
        IReadOnlyList<ChartLineData> series,
        IReadOnlyList<string> labels,
        IReadOnlyList<string>? fullLabels = null,
        IReadOnlyList<ChartReference>? references = null,
        IReadOnlyList<ChartMarkerData>? markers = null)
    {
        this.series = series;
        this.labels = labels;
        this.fullLabels = fullLabels ?? labels;
        this.references = references ?? [];
        this.markers = markers ?? [];
        Render();
    }

    private int PointCount => series.Count == 0 ? 0 : series.Max(line => line.Values.Count);

    private void OnPointerMoved(object sender, PointerRoutedEventArgs args)
    {
        var count = PointCount;
        if (count < 2)
        {
            return;
        }
        var plot = PlotRect;
        var position = args.GetCurrentPoint(Surface).Position;
        var ratio = (position.X - plot.Left) / plot.Width;
        var index = (int)Math.Round(Math.Clamp(ratio, 0, 1) * (count - 1));
        if (index != hovered)
        {
            hovered = index;
            Render();
        }
    }

    public override void Render()
    {
        Surface.Children.Clear();
        var count = PointCount;
        if (count == 0 || ActualWidth <= 0 || ActualHeight <= 0)
        {
            return;
        }

        var plot = PlotRect;
        var values = series.SelectMany(line => line.Values).Concat(references.Select(reference => reference.Value)).ToList();
        var ticks = ChartScale.NiceTicks(values.Count == 0 ? 0 : values.Min(), values.Count == 0 ? 1 : values.Max());
        var low = ticks[0];
        var high = ticks[^1];
        var span = Math.Abs(high - low) < double.Epsilon ? 1 : high - low;

        double X(int index) => count == 1 ? plot.Left + plot.Width / 2 : plot.Left + plot.Width * index / (count - 1.0);
        double Y(double value) => plot.Bottom - plot.Height * (value - low) / span;

        foreach (var tick in ticks)
        {
            var y = Y(tick);
            AddLine(plot.Left, y, plot.Right, y, GridBrush, 1, Dashes(3, 3));
            AddLabel(ChartScale.CompactLabel(tick), 0, y - 7, 10, SecondaryBrush, TextAlignment.Right, AxisWidth - 8);
        }

        foreach (var marker in markers.Where(marker => marker.Index >= 0 && marker.Index < count))
        {
            var x = X(marker.Index);
            AddLine(x, plot.Top, x, plot.Bottom, Palette.ToBrush(marker.ColorHex, Colors.Red), 2, Dashes(3, 3));
        }

        foreach (var reference in references)
        {
            var y = Y(reference.Value);
            AddLine(plot.Left, y, plot.Right, y, Palette.ToBrush(reference.ColorHex, Colors.Red), 1, reference.Dashed ? Dashes(6, 4) : null);
        }

        foreach (var line in series)
        {
            if (line.Values.Count == 0)
            {
                continue;
            }
            var color = Palette.ToColor(line.ColorHex, Colors.SteelBlue);
            var points = BuildPoints(line.Values, X, Y);
            if (line.Filled)
            {
                var area = new Polygon
                {
                    Points = [.. points, new Point(X(line.Values.Count - 1), plot.Bottom), new Point(X(0), plot.Bottom)],
                    Fill = new LinearGradientBrush
                    {
                        StartPoint = new Point(0, 0),
                        EndPoint = new Point(0, 1),
                        GradientStops =
                        [
                            new GradientStop { Color = Color.FromArgb(90, color.R, color.G, color.B), Offset = 0 },
                            new GradientStop { Color = Color.FromArgb(0, color.R, color.G, color.B), Offset = 1 },
                        ],
                    },
                };
                Surface.Children.Add(area);
            }
            var polyline = new Polyline
            {
                Points = [.. points],
                Stroke = new SolidColorBrush(color),
                StrokeThickness = line.Dashed ? 1.5 : 2,
                StrokeLineJoin = PenLineJoin.Round,
            };
            if (line.Dashed)
            {
                polyline.StrokeDashArray = Dashes(4, 3);
            }
            Surface.Children.Add(polyline);
        }

        var step = Math.Max((int)Math.Ceiling(count / Math.Max(plot.Width / 80, 2)), 1);
        for (var index = 0; index < count; index += step)
        {
            if (index < labels.Count)
            {
                AddLabel(labels[index], X(index) - 26, plot.Bottom + 4, 10, SecondaryBrush, TextAlignment.Center, 52);
            }
        }

        if (hovered is { } index2 && index2 < count)
        {
            var x = X(index2);
            AddLine(x, plot.Top, x, plot.Bottom, PrimaryBrush, 1);
            if (TooltipProvider is not null)
            {
                var (entries, footer) = TooltipProvider(index2);
                var title = index2 < fullLabels.Count ? fullLabels[index2] : string.Empty;
                var tooltip = BuildTooltip(title, entries, footer);
                Canvas.SetLeft(tooltip, Math.Clamp(x + 12, plot.Left, Math.Max(plot.Right - 240, plot.Left)));
                Canvas.SetTop(tooltip, plot.Top + 8);
                Surface.Children.Add(tooltip);
            }
        }
    }

    private List<Point> BuildPoints(IReadOnlyList<double> values, Func<int, double> x, Func<double, double> y)
    {
        var points = new List<Point>();
        for (var index = 0; index < values.Count; index++)
        {
            if (Stepped && index > 0)
            {
                points.Add(new Point(x(index), y(values[index - 1])));
            }
            points.Add(new Point(x(index), y(values[index])));
        }
        return points;
    }
}

/// <summary>Barres groupées (revenus contre dépenses).</summary>
public sealed class BarChartView : ChartViewBase
{
    private IReadOnlyList<ChartBarGroup> groups = [];
    private IReadOnlyList<string> colors = [];
    private int? hovered;

    public Func<int, (IEnumerable<TooltipEntry> Entries, string? Footer)>? TooltipProvider { get; set; }

    public BarChartView()
    {
        PointerMoved += (_, args) =>
        {
            if (groups.Count == 0)
            {
                return;
            }
            var plot = PlotRect;
            var slot = plot.Width / groups.Count;
            var index = Math.Clamp((int)((args.GetCurrentPoint(Surface).Position.X - plot.Left) / slot), 0, groups.Count - 1);
            if (index != hovered)
            {
                hovered = index;
                Render();
            }
        };
        PointerExited += (_, _) =>
        {
            hovered = null;
            Render();
        };
    }

    public void Update(IReadOnlyList<ChartBarGroup> groups, IReadOnlyList<string> colors)
    {
        this.groups = groups;
        this.colors = colors;
        Render();
    }

    public override void Render()
    {
        Surface.Children.Clear();
        if (groups.Count == 0 || ActualWidth <= 0 || ActualHeight <= 0)
        {
            return;
        }
        var plot = PlotRect;
        var maximum = groups.SelectMany(group => group.Values).DefaultIfEmpty(0).Max();
        var ticks = ChartScale.NiceTicks(0, Math.Max(maximum, 1));
        var high = ticks[^1];
        var slot = plot.Width / groups.Count;
        var seriesCount = Math.Max(groups[0].Values.Count, 1);
        var barWidth = Math.Clamp(slot * 0.8 / seriesCount, 2, 28);
        var labelStep = Math.Max((int)Math.Ceiling(groups.Count / Math.Max(plot.Width / 70, 1)), 1);

        foreach (var tick in ticks)
        {
            var y = plot.Bottom - plot.Height * tick / high;
            AddLine(plot.Left, y, plot.Right, y, GridBrush, 1, Dashes(3, 3));
            AddLabel(ChartScale.CompactLabel(tick), 0, y - 7, 10, SecondaryBrush, TextAlignment.Right, AxisWidth - 8);
        }

        for (var index = 0; index < groups.Count; index++)
        {
            var group = groups[index];
            var groupStart = plot.Left + slot * index + (slot - barWidth * seriesCount) / 2;
            for (var series = 0; series < group.Values.Count; series++)
            {
                var height = plot.Height * group.Values[series] / high;
                var color = Palette.ToColor(colors[series % colors.Count], Colors.SteelBlue);
                var bar = new Rectangle
                {
                    Width = barWidth,
                    Height = Math.Max(height, 0),
                    RadiusX = Math.Min(4, barWidth / 2),
                    RadiusY = Math.Min(4, barWidth / 2),
                    Fill = new SolidColorBrush(hovered is null || hovered == index
                        ? color
                        : Color.FromArgb(110, color.R, color.G, color.B)),
                };
                Canvas.SetLeft(bar, groupStart + barWidth * series);
                Canvas.SetTop(bar, plot.Bottom - Math.Max(height, 0));
                Surface.Children.Add(bar);
            }
            if (index % labelStep == 0)
            {
                AddLabel(group.Label, plot.Left + slot * index + slot / 2 - 30, plot.Bottom + 4, 10, SecondaryBrush, TextAlignment.Center, 60);
            }
        }

        if (hovered is { } hoveredIndex && TooltipProvider is not null && hoveredIndex < groups.Count)
        {
            var (entries, footer) = TooltipProvider(hoveredIndex);
            var tooltip = BuildTooltip(groups[hoveredIndex].Label, entries, footer);
            Canvas.SetLeft(tooltip, Math.Clamp(plot.Left + slot * hoveredIndex, plot.Left, Math.Max(plot.Right - 220, plot.Left)));
            Canvas.SetTop(tooltip, plot.Top + 8);
            Surface.Children.Add(tooltip);
        }
    }
}

/// <summary>Anneau de répartition ; les parts masquées ne comptent pas dans le total.</summary>
public sealed class DonutChartView : ChartViewBase
{
    private IReadOnlyList<DonutSliceData> slices = [];

    public double Thickness { get; set; } = 30;

    public string CenterTitle { get; set; } = string.Empty;

    public string CenterValue { get; set; } = string.Empty;

    public void Update(IReadOnlyList<DonutSliceData> slices, string centerTitle, string centerValue)
    {
        this.slices = slices;
        CenterTitle = centerTitle;
        CenterValue = centerValue;
        Render();
    }

    public override void Render()
    {
        Surface.Children.Clear();
        if (ActualWidth <= 0 || ActualHeight <= 0)
        {
            return;
        }
        var size = Math.Min(ActualWidth, ActualHeight);
        var center = new Point(ActualWidth / 2, ActualHeight / 2);
        var radius = (size - Thickness) / 2;
        if (radius <= 0)
        {
            return;
        }

        Surface.Children.Add(Ring(center, radius, 0, 1, GridBrush));

        var visible = slices.Where(slice => !slice.Hidden && slice.Value > 0).ToList();
        var total = visible.Sum(slice => slice.Value);
        if (total > 0)
        {
            var cursor = 0.0;
            var gap = visible.Count > 1 ? 0.004 : 0;
            foreach (var slice in visible)
            {
                var start = cursor + gap;
                cursor += slice.Value / total;
                var end = Math.Max(cursor - gap, start + 0.0001);
                Surface.Children.Add(Ring(center, radius, start, end, Palette.ToBrush(slice.ColorHex, Colors.Gray)));
            }
        }

        var stack = new StackPanel { HorizontalAlignment = HorizontalAlignment.Center };
        stack.Children.Add(new TextBlock
        {
            Text = CenterTitle.ToUpperInvariant(),
            FontSize = 10,
            FontWeight = Microsoft.UI.Text.FontWeights.Bold,
            Foreground = SecondaryBrush,
            HorizontalAlignment = HorizontalAlignment.Center,
        });
        stack.Children.Add(new TextBlock
        {
            Text = CenterValue,
            FontSize = 16,
            FontWeight = Microsoft.UI.Text.FontWeights.Bold,
            HorizontalAlignment = HorizontalAlignment.Center,
        });
        stack.Measure(new Size(size, size));
        Canvas.SetLeft(stack, center.X - stack.DesiredSize.Width / 2);
        Canvas.SetTop(stack, center.Y - stack.DesiredSize.Height / 2);
        Surface.Children.Add(stack);
    }

    private Path Ring(Point center, double radius, double start, double end, Brush brush)
    {
        if (end - start >= 0.999)
        {
            return new Path
            {
                Data = new EllipseGeometry { Center = center, RadiusX = radius, RadiusY = radius },
                Stroke = brush,
                StrokeThickness = Thickness,
                Fill = null,
            };
        }
        Point At(double fraction)
        {
            var angle = (fraction * 360 - 90) * Math.PI / 180;
            return new Point(center.X + radius * Math.Cos(angle), center.Y + radius * Math.Sin(angle));
        }
        var figure = new PathFigure { StartPoint = At(start), IsClosed = false, IsFilled = false };
        figure.Segments.Add(new ArcSegment
        {
            Point = At(end),
            Size = new Size(radius, radius),
            SweepDirection = SweepDirection.Clockwise,
            IsLargeArc = end - start > 0.5,
        });
        var geometry = new PathGeometry();
        geometry.Figures.Add(figure);
        return new Path
        {
            Data = geometry,
            Stroke = brush,
            StrokeThickness = Thickness,
            Fill = null,
        };
    }
}

/// <summary>Barre de progression colorée (budget).</summary>
public sealed class ProgressBarView : ContentControl
{
    private readonly Grid grid = new();
    private readonly Rectangle track = new();
    private readonly Rectangle fill = new();

    public ProgressBarView()
    {
        HorizontalContentAlignment = HorizontalAlignment.Stretch;
        track.Fill = (Brush)Application.Current.Resources["DmxSubtleBackground"];
        track.RadiusX = track.RadiusY = 4;
        fill.RadiusX = fill.RadiusY = 4;
        fill.HorizontalAlignment = HorizontalAlignment.Left;
        grid.Children.Add(track);
        grid.Children.Add(fill);
        Content = grid;
        Height = 8;
        SizeChanged += (_, _) => Apply();
    }

    public double Progress { get; private set; }

    public void Update(double progress, string colorHex)
    {
        Progress = progress;
        fill.Fill = Palette.ToBrush(colorHex, Colors.SteelBlue);
        Apply();
    }

    private void Apply()
    {
        track.Height = Height;
        fill.Height = Height;
        fill.Width = Math.Max(ActualWidth * Math.Clamp(Progress / 100, 0, 1), 0);
    }
}
