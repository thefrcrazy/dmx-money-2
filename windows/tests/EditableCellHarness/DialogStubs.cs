using System.ComponentModel;

namespace Microsoft.UI.Xaml
{
    public class DataTemplate { }
    public class Application
    {
        public static Application Current { get; } = new();
        public Dictionary<string, object> Resources { get; } = new() { ["WhatsNewTemplate"] = new DataTemplate() };
    }
}
namespace Microsoft.UI.Xaml.Automation
{
    public static class AutomationProperties { public static void SetName(DependencyObject element, string name) { } }
}
namespace Microsoft.UI.Xaml.Controls
{
    // Intentionally follows the WinUI inheritance boundary: a presenter is not a Control and has no IsEnabled.
    public class ContentPresenter : FrameworkElement { }
    public class StackPanel : FrameworkElement { public List<FrameworkElement> Children { get; } = new(); }
    public class ProgressRing : Control { public bool IsActive { get; set; } }
    public enum ScrollMode { Disabled }
    public enum ScrollBarVisibility { Disabled, Auto }
    public class ScrollViewer : ContentControl
    {
        public double MaxHeight { get; set; }
        public ScrollMode HorizontalScrollMode { get; set; }
        public ScrollBarVisibility HorizontalScrollBarVisibility { get; set; }
        public ScrollBarVisibility VerticalScrollBarVisibility { get; set; }
    }
    public enum ContentDialogButton { Primary, Close }
    public class ContentDialog : ContentControl
    {
        public string Title { get; set; } = "";
        public string PrimaryButtonText { get; set; } = "";
        public string CloseButtonText { get; set; } = "";
        public ContentDialogButton DefaultButton { get; set; }
        public bool IsPrimaryButtonEnabled { get; set; }
        public event Action<ContentDialog, ContentDialogClosingEventArgs>? Closing;
        public event Action<ContentDialog, ContentDialogButtonClickEventArgs>? PrimaryButtonClick;
        public event Action<ContentDialog, EventArgs>? Closed;
        public ContentDialogClosingEventArgs RaiseClosing()
        {
            var args = new ContentDialogClosingEventArgs(); Closing?.Invoke(this, args); return args;
        }
        public ContentDialogButtonClickEventArgs RaisePrimary()
        {
            var args = new ContentDialogButtonClickEventArgs(); PrimaryButtonClick?.Invoke(this, args); return args;
        }
        public void RaiseClosed() => Closed?.Invoke(this, EventArgs.Empty);
    }
    public class ContentDialogClosingEventArgs { public bool Cancel { get; set; } }
    public class ContentDialogButtonClickEventArgs
    {
        private readonly TestDeferral deferral = new();
        public bool Cancel { get; set; }
        public bool Deferred { get; private set; }
        public TestDeferral GetDeferral() { Deferred = true; return deferral; }
        public Task Completion => Deferred ? deferral.Completion : Task.CompletedTask;
    }
    public class TestDeferral
    {
        private readonly TaskCompletionSource complete = new(TaskCreationOptions.RunContinuationsAsynchronously);
        public Task Completion => complete.Task;
        public void Complete() => complete.TrySetResult();
    }
}
namespace DmxMoney.ViewModels
{
    public class FormViewModel : INotifyPropertyChanged
    {
        public event PropertyChangedEventHandler? PropertyChanged;
        public virtual string Title => "Fixture";
        public virtual string SubmitTitle => "Enregistrer";
        public virtual bool ShowsSubmit => true;
        public string? Error { get; private set; }
        public bool IsBusy { get; private set; }
        public bool IsSubmitting { get; private set; }
        public bool IsClosed { get; private set; }
        public virtual Task<bool> SubmitAsync() => Task.FromResult(true);
        public virtual void Close() => IsClosed = true;
        public void SetBusy(bool value, bool submitting = false)
        {
            IsBusy = value; IsSubmitting = submitting; PropertyChanged?.Invoke(this, new(nameof(IsBusy)));
        }
        public void ReportError(Exception exception)
        {
            Error = exception.Message; PropertyChanged?.Invoke(this, new(nameof(Error)));
        }
    }
    public class AccountFormViewModel : FormViewModel { }
    public class AccountGroupsViewModel : FormViewModel { }
    public class CategoryFormViewModel : FormViewModel { }
    public class TransactionFormViewModel : FormViewModel { }
    public class BudgetFormViewModel : FormViewModel { }
    public class ScheduledFormViewModel : FormViewModel { }
    public class FakeTransactionFormViewModel : FormViewModel { }
    public class BudgetSuggestionsViewModel : FormViewModel { }
    public class ScheduledSuggestionsViewModel : FormViewModel { }
    public class RestoreBackupViewModel : FormViewModel { }
    public class StatementImportViewModel : FormViewModel { }
}
namespace DmxMoney.App
{
    public static class Palette { public static object Expense => new(); }
}
