using DmxMoney.Interop;
using DmxMoney.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Windows.System;

namespace DmxMoney.App;

/// <summary>
/// Cellule du journal éditable au double-clic (description, montant), comme en 1.x.
/// La valeur est écrite par le noyau ; l'affichage revient à la valeur d'origine si la saisie est refusée.
/// </summary>
public sealed class EditableCell : ContentControl
{
    public static readonly DependencyProperty RowProperty = DependencyProperty.Register(
        nameof(Row), typeof(object), typeof(EditableCell), new PropertyMetadata(null, (sender, _) => ((EditableCell)sender).Apply()));

    public static readonly DependencyProperty FieldProperty = DependencyProperty.Register(
        nameof(Field), typeof(string), typeof(EditableCell), new PropertyMetadata("description", (sender, _) => ((EditableCell)sender).Apply()));

    private readonly TextBlock label = new() { VerticalAlignment = VerticalAlignment.Center, TextTrimming = TextTrimming.CharacterEllipsis };
    private readonly TextBox editor = new() { Visibility = Visibility.Collapsed, BorderThickness = new Thickness(1), Padding = new Thickness(4, 2, 4, 2) };

    public object? Row
    {
        get => GetValue(RowProperty);
        set => SetValue(RowProperty, value);
    }

    public string Field
    {
        get => (string)GetValue(FieldProperty);
        set => SetValue(FieldProperty, value);
    }

    public EditableCell()
    {
        var grid = new Grid();
        grid.Children.Add(label);
        grid.Children.Add(editor);
        Content = grid;
        HorizontalContentAlignment = HorizontalAlignment.Stretch;
        DoubleTapped += (_, args) =>
        {
            args.Handled = true;
            BeginEdit();
        };
        editor.LostFocus += (_, _) => Commit();
        editor.KeyDown += OnEditorKeyDown;
    }

    private JournalRow? JournalRow => Row as JournalRow;

    private void Apply()
    {
        if (JournalRow is not { } row)
        {
            label.Text = string.Empty;
            return;
        }
        if (Field == "amount")
        {
            label.Text = Money.Signed(row.Transaction.Amount, row.Transaction.TransactionType);
            label.Foreground = Format.IncomeExpenseBrush(row.Transaction.TransactionType);
            label.FontWeight = Microsoft.UI.Text.FontWeights.SemiBold;
            label.HorizontalAlignment = HorizontalAlignment.Right;
            editor.TextAlignment = TextAlignment.Right;
        }
        else
        {
            label.Text = row.Transaction.Description;
            label.FontWeight = Microsoft.UI.Text.FontWeights.Medium;
        }
    }

    private void BeginEdit()
    {
        if (JournalRow is not { } row)
        {
            return;
        }
        editor.Text = Field == "amount" ? AmountInput.Text(row.Transaction.Amount, emptyWhenZero: false) : row.Transaction.Description;
        editor.Visibility = Visibility.Visible;
        label.Visibility = Visibility.Collapsed;
        editor.SelectAll();
        editor.Focus(FocusState.Programmatic);
    }

    private void OnEditorKeyDown(object sender, KeyRoutedEventArgs args)
    {
        if (args.Key == VirtualKey.Enter)
        {
            args.Handled = true;
            Commit();
        }
        else if (args.Key == VirtualKey.Escape)
        {
            args.Handled = true;
            EndEdit();
        }
    }

    private void Commit()
    {
        if (editor.Visibility != Visibility.Visible)
        {
            return;
        }
        var text = editor.Text.Trim();
        EndEdit();
        if (JournalRow is not { } row || App.Shell is null)
        {
            return;
        }
        var journal = App.Shell.Journal;
        if (Field == "amount")
        {
            journal.UpdateAmount(row.Transaction.Id, text);
        }
        else if (text.Length > 0 && text != row.Transaction.Description)
        {
            journal.UpdateDescription(row.Transaction.Id, text);
        }
    }

    private void EndEdit()
    {
        editor.Visibility = Visibility.Collapsed;
        label.Visibility = Visibility.Visible;
        Apply();
    }
}
