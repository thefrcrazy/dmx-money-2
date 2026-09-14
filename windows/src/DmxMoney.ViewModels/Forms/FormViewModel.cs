using CommunityToolkit.Mvvm.ComponentModel;
using DmxMoney.Interop;

namespace DmxMoney.ViewModels;

/// <summary>Base des formulaires : titre, message d'erreur, validation.</summary>
public abstract class FormViewModel : ObservableObject
{
    private string? error;

    protected EngineStore Store { get; }

    protected FormViewModel(EngineStore store) => Store = store;

    public string? Error
    {
        get => error;
        protected set => SetProperty(ref error, value);
    }

    public abstract string Title { get; }

    public virtual string SubmitTitle => "Enregistrer";

    public virtual bool ShowsSubmit => true;

    /// <summary>Valide le formulaire ; renvoie <c>true</c> si la boîte de dialogue peut se fermer.</summary>
    public abstract bool Submit();
}

/// <summary>Construit le formulaire correspondant à une demande (brouillon préparé par le noyau).</summary>
public static class FormFactory
{
    public static FormViewModel? Create(EngineStore store, FormRequest request)
    {
        switch (request)
        {
            case FormRequest.AccountForm form:
                var accountDraft = store.Read(engine => engine.AccountDraft(form.Id));
                return accountDraft is null ? null : new AccountFormViewModel(store, accountDraft);
            case FormRequest.AccountGroups:
                return new AccountGroupsViewModel(store);
            case FormRequest.CategoryForm form:
                var categoryDraft = store.Read(engine => engine.CategoryDraft(form.Id));
                return categoryDraft is null ? null : new CategoryFormViewModel(store, categoryDraft);
            case FormRequest.TransactionForm form:
                var transactionDraft = store.Read(engine => engine.TransactionDraft(form.Id, [.. store.SelectedAccountIds], store.Today));
                return transactionDraft is null ? null : new TransactionFormViewModel(store, transactionDraft);
            case FormRequest.BudgetForm form:
                var budgetDraft = store.Read(engine => engine.BudgetDraft(form.Id, [.. store.SelectedAccountIds]));
                if (budgetDraft is null)
                {
                    return null;
                }
                if (form.CategoryId is { } categoryId)
                {
                    var name = budgetDraft.Name.Length == 0 ? store.CategoryById(categoryId)?.Name ?? string.Empty : budgetDraft.Name;
                    budgetDraft = budgetDraft with { CategoryId = categoryId, Name = name };
                }
                return new BudgetFormViewModel(store, budgetDraft);
            case FormRequest.ScheduledForm form:
                var scheduledDraft = store.Read(engine => engine.ScheduledDraft(form.Id, store.Today));
                return scheduledDraft is null ? null : new ScheduledFormViewModel(store, scheduledDraft);
            case FormRequest.FakeTransactionForm form:
                var fakeDraft = store.Read(engine => engine.FakeTransactionDraft(form.Id, [.. store.SelectedAccountIds], store.Today));
                return fakeDraft is null ? null : new FakeTransactionFormViewModel(store, fakeDraft);
            case FormRequest.BudgetSuggestions:
                return new BudgetSuggestionsViewModel(store);
            case FormRequest.ScheduledSuggestions:
                return new ScheduledSuggestionsViewModel(store);
            case FormRequest.RestoreBackup form:
                return new RestoreBackupViewModel(store, form.Content, form.FileName);
            case FormRequest.StatementImport form:
                return new StatementImportViewModel(store, form.Content, form.FileName);
            case FormRequest.WhatsNew:
                return new WhatsNewViewModel(store);
            default:
                return null;
        }
    }
}
