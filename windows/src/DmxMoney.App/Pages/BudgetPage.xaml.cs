using System.ComponentModel;
using DmxMoney.Interop;
using DmxMoney.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;

namespace DmxMoney.App;

public sealed partial class BudgetPage : Page
{
    public BudgetViewModel ViewModel { get; private set; } = null!;

    private bool syncing;

    public BudgetPage()
    {
        InitializeComponent();
        NavigationCacheMode = NavigationCacheMode.Required;
    }

    protected override void OnNavigatedTo(NavigationEventArgs args)
    {
        var shell = (ShellViewModel)args.Parameter;
        if (!ReferenceEquals(ViewModel, shell.Budget))
        {
            if (ViewModel is not null)
            {
                ViewModel.PropertyChanged -= OnViewModelChanged;
            }
            ViewModel = shell.Budget;
            ViewModel.PropertyChanged += OnViewModelChanged;
            syncing = true;
            CategoriesList.ItemsSource = shell.Store.SelectableCategories;
            syncing = false;
        }
        Bindings?.Update();
        UpdateVisuals();
    }

    private void OnViewModelChanged(object? sender, PropertyChangedEventArgs args) => UpdateVisuals();

    private void UpdateVisuals()
    {
        if (ViewModel.View is not { } view)
        {
            return;
        }
        Subtitle.Text = $"{view.MonthLabel} · budgets configurés et dépenses du Journal";

        PlannedValue.Text = Money.Rounded(view.TotalBudgeted);
        PlannedCaption.Text = $"{Plural.Of((int)view.BudgetCount, "budget")} {(view.BudgetCount > 1 ? "configurés" : "configuré")}";
        SpentValue.Text = Money.Rounded(view.TotalSpent);
        SpentCaption.Text = $"{Plural.Of((int)view.ExpenseCount, "dépense")} ce mois-ci";
        RemainingValue.Text = Money.Rounded(view.Remaining);
        RemainingValue.Foreground = Format.PositiveBrush(view.Remaining);
        RemainingIcon.Foreground = Format.PositiveBrush(view.Remaining);
        RemainingCaption.Text = $"{Money.Rounded(view.RemainingPerDay)} / jour restant";

        StateValue.Text = ViewModel.StateLabel;
        StateCaption.Text = ViewModel.PaceLabel;
        (StateIcon.Icon, var stateBrush) = view.State switch
        {
            BudgetState.ToConfigure => ("CalendarClock", Palette.Resource("TextFillColorSecondaryBrush")),
            BudgetState.UnderControl => ("CheckCircle2", Palette.Income),
            _ => ("AlertCircle", Palette.Expense),
        };
        StateIcon.Foreground = stateBrush;
        StateValue.Foreground = stateBrush;

        ProgressLabel.Text = $"{Format.PercentTight(view.Progress)} utilisé";
        ProgressBar.Update(view.Progress, view.Progress > 100 ? "#ef4444" : "#10b981");
        var details = new List<string>
        {
            $"{Money.Format(view.TotalSpent)} dépensés",
            $"{Money.Format(view.TotalBudgeted)} prévus",
            $"{Money.Format(view.ExpectedSpend)} théoriques au {view.TodayLabel}",
        };
        if (view.OverBudgetCount > 0)
        {
            details.Add(Plural.Of((int)view.OverBudgetCount, "dépassement"));
        }
        if (view.UnbudgetedCount > 0)
        {
            details.Add($"{Plural.Of((int)view.UnbudgetedCount, "catégorie")} non {(view.UnbudgetedCount > 1 ? "budgétées" : "budgétée")}");
        }
        ProgressDetails.Text = string.Join("   ·   ", details);

        var budgeted = view.Categories.Count(row => !row.IsUnbudgeted);
        CategoryCount.Text = budgeted == view.BudgetedCategoryCount
            ? $"{view.BudgetedCategoryCount} {(view.BudgetedCategoryCount > 1 ? "catégories budgétées" : "catégorie budgétée")}"
            : $"{budgeted} / {view.BudgetedCategoryCount} catégories budgétées";

        CategoryRows.ItemsSource = view.Categories;
        EmptyState.Visibility = view.Categories.Length == 0 ? Visibility.Visible : Visibility.Collapsed;
        EmptyTitle.Text = view.BudgetedCategoryCount == 0 ? "Aucun budget configuré" : "Aucun budget ne correspond aux filtres.";
        EmptyMessage.Text = view.BudgetedCategoryCount == 0
            ? "Crée une enveloppe mensuelle, même sans échéance liée."
            : "Modifie la recherche ou les catégories sélectionnées.";
        EmptyAction.Visibility = view.BudgetedCategoryCount == 0 ? Visibility.Visible : Visibility.Collapsed;

        SuggestionsButton.Visibility = ViewModel.SuggestionCount > 0 ? Visibility.Visible : Visibility.Collapsed;
        SuggestionsLabel.Text = $"Suggestions ({ViewModel.SuggestionCount})";
        CategoriesButton.Content = ViewModel.Categories.Count == 0 ? "Toutes les catégories" : $"Catégories ({ViewModel.Categories.Count})";
    }

    private void OnCategoriesChanged(object sender, SelectionChangedEventArgs args)
    {
        if (!syncing)
        {
            ViewModel.Categories = [.. CategoriesList.SelectedItems.OfType<Category>().Select(category => category.Id)];
        }
    }

    private void OnCreateBudgetForCategory(object sender, RoutedEventArgs args)
    {
        if (sender is FrameworkElement { Tag: string categoryId })
        {
            ViewModel.NewBudgetForCategoryCommand.Execute(categoryId);
        }
    }

    private void OnEditBudget(object sender, RoutedEventArgs args)
    {
        if (sender is FrameworkElement { Tag: string id })
        {
            ViewModel.EditCommand.Execute(id);
        }
    }

    private void OnDeleteBudget(object sender, RoutedEventArgs args)
    {
        if (sender is FrameworkElement { Tag: string id })
        {
            ViewModel.DeleteCommand.Execute(id);
        }
    }
}
