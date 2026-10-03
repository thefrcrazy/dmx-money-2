using System.ComponentModel;
using DmxMoney.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace DmxMoney.App;

/// <summary>
/// Hôte des formulaires : titre, contenu choisi selon le modèle de vue, message d'erreur,
/// et bouton principal qui ne ferme la boîte que si la validation passe.
/// </summary>
public sealed class FormDialog : ContentDialog
{
    /// <summary>Formulaire actuellement affiché (utilisé par les gabarits pour leurs actions).</summary>
    public static FormViewModel? Current { get; private set; }

    private readonly FormViewModel model;
    private bool submitting;
    private readonly ContentPresenter presenter;
    private readonly ProgressRing progress = new() { Width = 20, Height = 20 };
    private readonly TextBlock error = new()
    {
        Foreground = Palette.Expense,
        FontSize = 12,
        TextWrapping = TextWrapping.Wrap,
        Visibility = Visibility.Collapsed,
        Margin = new Thickness(0, 10, 0, 0),
    };

    public FormDialog(FormViewModel model)
    {
        this.model = model;
        Current = model;
        Title = model.Title;
        PrimaryButtonText = model.ShowsSubmit ? model.SubmitTitle : string.Empty;
        CloseButtonText = model.ShowsSubmit ? "Annuler" : "Fermer";
        DefaultButton = model.ShowsSubmit ? ContentDialogButton.Primary : ContentDialogButton.Close;

        presenter = new ContentPresenter
        {
            Content = model,
            ContentTemplate = TemplateFor(model),
            HorizontalContentAlignment = HorizontalAlignment.Stretch,
        };
        var stack = new StackPanel();
        stack.Children.Add(presenter);
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(progress, "Traitement en cours");
        stack.Children.Add(progress);
        stack.Children.Add(error);
        Content = new ScrollViewer
        {
            Content = stack,
            MaxHeight = 560,
            HorizontalScrollMode = ScrollMode.Disabled,
            HorizontalScrollBarVisibility = ScrollBarVisibility.Disabled,
            VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
        };

        error.Text = model.Error ?? string.Empty;
        error.Visibility = model.Error is null ? Visibility.Collapsed : Visibility.Visible;
        IsPrimaryButtonEnabled = !model.IsBusy;
        presenter.IsEnabled = !model.IsBusy;
        progress.IsActive = model.IsBusy;
        progress.Visibility = model.IsBusy ? Visibility.Visible : Visibility.Collapsed;
        model.PropertyChanged += OnModelChanged;
        Closing += (_, args) => args.Cancel = submitting || model.IsSubmitting;
        PrimaryButtonClick += OnPrimary;
        Closed += (_, _) =>
        {
            model.PropertyChanged -= OnModelChanged;
            model.Close();
            if (ReferenceEquals(Current, model))
            {
                Current = null;
            }
        };
    }

    private void OnModelChanged(object? sender, PropertyChangedEventArgs args)
    {
        switch (args.PropertyName)
        {
            case nameof(FormViewModel.IsBusy):
                IsPrimaryButtonEnabled = !submitting && !model.IsBusy;
                presenter.IsEnabled = !model.IsBusy;
                progress.IsActive = model.IsBusy;
                progress.Visibility = model.IsBusy ? Visibility.Visible : Visibility.Collapsed;
                break;
            case nameof(FormViewModel.Error):
                error.Text = model.Error ?? string.Empty;
                error.Visibility = model.Error is null ? Visibility.Collapsed : Visibility.Visible;
                break;
            case nameof(FormViewModel.SubmitTitle):
                PrimaryButtonText = model.SubmitTitle;
                break;
            case nameof(FormViewModel.Title):
                Title = model.Title;
                break;
        }
    }

    private async void OnPrimary(ContentDialog sender, ContentDialogButtonClickEventArgs args)
    {
        if (submitting || model.IsBusy) { args.Cancel = true; return; }
        var deferral = args.GetDeferral();
        submitting = true;
        IsPrimaryButtonEnabled = false;
        try { args.Cancel = !await model.SubmitAsync(); }
        catch (Exception exception) { model.ReportError(exception); args.Cancel = true; }
        finally
        {
            submitting = false;
            IsPrimaryButtonEnabled = !model.IsBusy;
            presenter.IsEnabled = !model.IsBusy;
            deferral.Complete();
        }
    }

    private static DataTemplate TemplateFor(FormViewModel model)
    {
        var key = model switch
        {
            AccountFormViewModel => "AccountFormTemplate",
            AccountGroupsViewModel => "AccountGroupsTemplate",
            CategoryFormViewModel => "CategoryFormTemplate",
            TransactionFormViewModel => "TransactionFormTemplate",
            BudgetFormViewModel => "BudgetFormTemplate",
            ScheduledFormViewModel => "ScheduledFormTemplate",
            FakeTransactionFormViewModel => "FakeTransactionFormTemplate",
            BudgetSuggestionsViewModel => "BudgetSuggestionsTemplate",
            ScheduledSuggestionsViewModel => "ScheduledSuggestionsTemplate",
            RestoreBackupViewModel => "RestoreBackupTemplate",
            StatementImportViewModel => "StatementImportTemplate",
            _ => "WhatsNewTemplate",
        };
        return (DataTemplate)Application.Current.Resources[key];
    }
}
