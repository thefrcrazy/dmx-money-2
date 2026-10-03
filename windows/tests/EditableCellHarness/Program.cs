// Executes the real cell class with narrow WinUI event stubs; this does not emulate visual recycling.
using System.Reflection;
using DmxMoney.App;
using DmxMoney.Interop;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Windows.System;

static void Call(EditableCell cell, string method, params object?[] arguments)
    => typeof(EditableCell).GetMethod(method, BindingFlags.Instance | BindingFlags.NonPublic)!.Invoke(cell, arguments);
static TextBox Editor(EditableCell cell) => (TextBox)typeof(EditableCell).GetField("editor", BindingFlags.Instance | BindingFlags.NonPublic)!.GetValue(cell)!;
static JournalRow Row(string id, string description = "original", double amount = 10) => new(new Transaction(id, description, amount, "expense"));
static void Require(bool value, string message) { if (!value) throw new Exception(message); }

var cases = 0;
foreach (var field in new[] { "description", "amount" })
{
    foreach (var replacement in new object?[] { Row("B"), null })
    {
        App.Shell = new TestShell();
        var cell = new EditableCell { Field = field, Row = Row("A") };
        Call(cell, "BeginEdit"); Editor(cell).Text = field == "amount" ? "987" : "draft A";
        cell.Row = replacement; Call(cell, "Commit");
        Require(App.Shell.Journal.Writes.Count == 0, "A recycled/unbound cell committed a stale draft"); cases++;
    }
    App.Shell = new TestShell();
    var unchanged = new EditableCell { Field = field, Row = Row("A") };
    Call(unchanged, "BeginEdit"); unchanged.Row = Row("A", "remote", 25); Call(unchanged, "Commit");
    Require(App.Shell.Journal.Writes.Count == 0, "An unchanged draft overwrote a remote value"); cases++;

    App.Shell = new TestShell();
    var cancel = new EditableCell { Field = field, Row = Row("A") };
    Call(cancel, "BeginEdit"); Editor(cancel).Text = "draft";
    Call(cancel, "OnEditorKeyDown", Editor(cancel), new KeyRoutedEventArgs { Key = VirtualKey.Escape });
    Call(cancel, "Commit"); Require(App.Shell.Journal.Writes.Count == 0, "Escape committed a draft"); cases++;

    App.Shell = new TestShell();
    var edited = new EditableCell { Field = field, Row = Row("A") };
    Call(edited, "BeginEdit"); Editor(edited).Text = field == "amount" ? "15" : "local";
    edited.Row = Row("A", "remote", 25); Call(edited, "Commit"); Call(edited, "Commit");
    var write = App.Shell.Journal.Writes.Single();
    Require(write.Id == "A", "Commit lost its original ID");
    Require(write.Base == (field == "amount" ? "10" : "original"), "Commit lost its original field value"); cases++;
}
App.Shell = new TestShell();
var changedField = new EditableCell { Field = "description", Row = Row("A") };
Call(changedField, "BeginEdit"); Editor(changedField).Text = "12"; changedField.Field = "amount"; Call(changedField, "Commit");
Require(App.Shell.Journal.Writes.Count == 0, "Changing the field committed an old draft"); cases++;
Console.WriteLine($"EditableCell: {cases} guard/cancellation/baseline cases passed against the real class.");
