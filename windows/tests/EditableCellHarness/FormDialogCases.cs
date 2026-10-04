using System.Reflection;
using DmxMoney.App;
using DmxMoney.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

// The actual FormDialog.cs is linked into this executable. This checks async control flow, not WinUI rendering.
static class FormDialogCases
{
    static T Field<T>(FormDialog dialog, string name) => (T)typeof(FormDialog).GetField(name, BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(dialog)!;
    static void Require(bool value, string message) { if (!value) throw new Exception(message); }
    sealed class DeferredForm : FormViewModel
    {
        public readonly TaskCompletionSource<bool> Result = new(TaskCreationOptions.RunContinuationsAsynchronously);
        public int Calls;
        public override async Task<bool> SubmitAsync()
        {
            Calls++; SetBusy(true, true);
            try { return await Result.Task; }
            finally { SetBusy(false); }
        }
    }
    public static async Task Run()
    {
        var model = new DeferredForm();
        var dialog = new FormDialog(model);
        var host = Field<ContentControl>(dialog, "presenter");
        Require(ReferenceEquals(host.Content, model) && host.ContentTemplate is not null && !host.IsTabStop, "Form host changed its model/template or added a keyboard stop");
        var success = dialog.RaisePrimary();
        Require(success.Deferred && !success.Completion.IsCompleted && !dialog.IsPrimaryButtonEnabled && !host.IsEnabled, "Submission did not defer closing and disable inputs");
        Require(dialog.RaiseClosing().Cancel, "A write could close the form before its result");
        var duplicate = dialog.RaisePrimary();
        Require(duplicate.Cancel && !duplicate.Deferred && model.Calls == 1, "Double submission was not blocked");
        model.Result.SetResult(true); await success.Completion.WaitAsync(TimeSpan.FromSeconds(5));
        Require(!success.Cancel && host.IsEnabled && dialog.IsPrimaryButtonEnabled && !dialog.RaiseClosing().Cancel, "Successful submission did not permit closure");
        dialog.RaiseClosed();
        Require(model.IsClosed && FormDialog.Current is null, "Closed form retained its active model");

        var failing = new DeferredForm(); var failureDialog = new FormDialog(failing);
        var failure = failureDialog.RaisePrimary();
        failing.Result.SetException(new IOException("Fixture import failed")); await failure.Completion.WaitAsync(TimeSpan.FromSeconds(5));
        Require(failure.Cancel && !failing.IsClosed && Field<ContentControl>(failureDialog, "presenter").IsEnabled, "Failed import closed or left inputs disabled");
        Require(Field<TextBlock>(failureDialog, "error").Text == "Fixture import failed" && Field<TextBlock>(failureDialog, "error").Visibility == Visibility.Visible, "Import error is not visible in the retained dialog");
        failureDialog.RaiseClosed();

        var preparing = new FormViewModel(); preparing.SetBusy(true);
        var preparingDialog = new FormDialog(preparing);
        Require(!Field<ContentControl>(preparingDialog, "presenter").IsEnabled && Field<ProgressRing>(preparingDialog, "progress").IsActive && !preparingDialog.RaiseClosing().Cancel, "Preparation must disable inputs but remain cancellable");
        preparingDialog.RaiseClosed();
        preparing.SetBusy(false);
        Require(!Field<ContentControl>(preparingDialog, "presenter").IsEnabled, "A closed dialog retained its property-change subscription");
        Console.WriteLine("FormDialog: 10 host/deferral/busy/error/closing/lifecycle cases passed against the real class (WinUI event stubs).");
    }
}
