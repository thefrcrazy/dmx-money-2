using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using DmxMoney.Interop;

namespace DmxMoney.ViewModels;

public sealed partial class AccountsViewModel : PageViewModel
{
    [ObservableProperty]
    private AccountsView? view;

    [ObservableProperty]
    private string search = string.Empty;

    [ObservableProperty]
    private IReadOnlyList<string> types = [];

    public AccountsViewModel(EngineStore store) : base(store) => Refresh();

    public IReadOnlyList<string> AccountTypes { get; } = DmxFfiMethods.AccountTypes();

    public IReadOnlyList<string> CustomGroups => Store.Settings.CustomGroups;

    public override void Refresh()
    {
        var query = new AccountsQuery(Search, [.. Types]);
        View = Store.Read(engine => engine.Accounts(query));
    }

    partial void OnSearchChanged(string value) => Refresh();

    partial void OnTypesChanged(IReadOnlyList<string> value) => Refresh();

    [RelayCommand]
    private void NewAccount() => Store.Present(new FormRequest.AccountForm(null));

    [RelayCommand]
    private void ManageGroups() => Store.Present(new FormRequest.AccountGroups());

    [RelayCommand]
    private void EditAccount(string accountId) => Store.Present(new FormRequest.AccountForm(accountId));

    [RelayCommand]
    private void DeleteAccount(string accountId) => Store.Confirm(
        "Supprimer le compte",
        "Êtes-vous sûr de vouloir supprimer ce compte ? Cette action est irréversible et supprimera toutes les transactions associées.",
        () => Store.Run(engine => engine.DeleteAccount(accountId), "Compte supprimé"));

    /// <summary>Glisser-déposer : compte déposé sur un autre compte.</summary>
    [RelayCommand]
    private void MoveAccountOnAccount((string AccountId, string TargetId) move) => Store.Run(
        engine => engine.MoveAccount(move.AccountId, new DropTarget.OnAccount(move.TargetId)));

    /// <summary>Glisser-déposer : compte déposé sur un groupe (<c>null</c> = Non groupés).</summary>
    [RelayCommand]
    private void MoveAccountToGroup((string AccountId, string? Group) move) => Store.Run(
        engine => engine.MoveAccount(move.AccountId, new DropTarget.OnGroup(move.Group)));

    [RelayCommand]
    private void MoveGroup((string Group, string OverGroup) move) => Store.Run(
        engine => engine.MoveGroup(move.Group, move.OverGroup));
}
