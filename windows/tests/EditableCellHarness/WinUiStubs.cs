namespace Microsoft.UI.Xaml
{
    public enum VerticalAlignment { Center }
    public enum HorizontalAlignment { Stretch, Right }
    public enum TextTrimming { CharacterEllipsis }
    public enum Visibility { Visible, Collapsed }
    public enum FocusState { Programmatic }
    public readonly record struct Thickness(double A, double B = 0, double C = 0, double D = 0);
    public sealed class DependencyProperty(string name, PropertyMetadata metadata)
    {
        public PropertyMetadata Metadata => metadata;
        public static DependencyProperty Register(string name, Type type, Type owner, PropertyMetadata metadata) => new(name, metadata);
    }
    public sealed class DependencyPropertyChangedEventArgs : EventArgs { }
    public sealed class PropertyMetadata(object? value, Action<DependencyObject, DependencyPropertyChangedEventArgs> callback)
    {
        public object? Value => value;
        public Action<DependencyObject, DependencyPropertyChangedEventArgs> Callback => callback;
    }
    public class DependencyObject
    {
        private readonly Dictionary<DependencyProperty, object?> values = new();
        public object? GetValue(DependencyProperty property) => values.GetValueOrDefault(property, property.Metadata.Value);
        public void SetValue(DependencyProperty property, object? value) { values[property] = value; property.Metadata.Callback(this, new()); }
    }
    public class FrameworkElement : DependencyObject
    {
        public VerticalAlignment VerticalAlignment { get; set; }
        public HorizontalAlignment HorizontalAlignment { get; set; }
        public Visibility Visibility { get; set; }
        public event EventHandler<Microsoft.UI.Xaml.Input.DoubleTappedRoutedEventArgs>? DoubleTapped;
    }
}
namespace Microsoft.UI.Xaml.Controls
{
    public enum TextAlignment { Right }
    public class ContentControl : Microsoft.UI.Xaml.FrameworkElement { public object? Content { get; set; } public Microsoft.UI.Xaml.HorizontalAlignment HorizontalContentAlignment { get; set; } }
    public class Grid : Microsoft.UI.Xaml.FrameworkElement { public List<object> Children { get; } = new(); }
    public class TextBlock : Microsoft.UI.Xaml.FrameworkElement
    {
        public string Text { get; set; } = "";
        public object? Foreground { get; set; }
        public object? FontWeight { get; set; }
        public Microsoft.UI.Xaml.TextTrimming TextTrimming { get; set; }
    }
    public class TextBox : Microsoft.UI.Xaml.FrameworkElement
    {
        public string Text { get; set; } = "";
        public Microsoft.UI.Xaml.Thickness BorderThickness { get; set; }
        public Microsoft.UI.Xaml.Thickness Padding { get; set; }
        public TextAlignment TextAlignment { get; set; }
        public event EventHandler? LostFocus;
        public event EventHandler<Microsoft.UI.Xaml.Input.KeyRoutedEventArgs>? KeyDown;
        public void SelectAll() { }
        public bool Focus(Microsoft.UI.Xaml.FocusState state) => true;
    }
}
namespace Microsoft.UI.Xaml.Input
{
    public class DoubleTappedRoutedEventArgs : EventArgs { public bool Handled { get; set; } }
    public class KeyRoutedEventArgs : EventArgs { public Windows.System.VirtualKey Key { get; set; } public bool Handled { get; set; } }
}
namespace Microsoft.UI.Xaml.Media { public class Brush { } }
namespace Microsoft.UI.Text { public static class FontWeights { public static object SemiBold => new(); public static object Medium => new(); } }
namespace Windows.System { public enum VirtualKey { Enter, Escape } }
namespace DmxMoney.Interop
{
    public record Transaction(string Id, string Description, double Amount, string TransactionType);
    public record JournalRow(Transaction Transaction);
}
namespace DmxMoney.ViewModels
{
    public static class Money { public static string Signed(double amount, string type) => amount.ToString(); }
    public static class AmountInput { public static string Text(double amount, bool emptyWhenZero) => amount.ToString(); }
}
namespace DmxMoney.App
{
    public static class Format { public static object IncomeExpenseBrush(string type) => new(); }
    public static class App { public static TestShell? Shell { get; set; } }
    public class TestShell { public TestJournal Journal { get; } = new(); }
    public record Write(string Field, string Id, string Value, string? Base);
    public class TestJournal
    {
        public List<Write> Writes { get; } = new();
        public void UpdateAmount(string id, string text, double? baseline = null) => Writes.Add(new("amount", id, text, baseline?.ToString(System.Globalization.CultureInfo.InvariantCulture)));
        public void UpdateDescription(string id, string text, string? baseline = null) => Writes.Add(new("description", id, text, baseline));
    }
}
