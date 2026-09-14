using DmxMoney.Interop;

namespace DmxMoney.Tests;

public class InteropSmokeTests
{
    private const string Today = "2026-09-11";

    [Fact]
    public void InMemoryEngineSeedsDefaultsAndComputesDashboard()
    {
        using var engine = DmxEngine.OpenInMemory();
        Assert.False(string.IsNullOrEmpty(DmxFfiMethods.CoreVersion()));
        Assert.Contains(engine.Categories(), category => category.Id == "transfer");

        var accountId = engine.SaveAccount(engine.AccountDraft(null) with
        {
            Name = "Compte courant",
            InitialBalance = 1200,
        });

        var transaction = engine.TransactionDraft(null, [], Today) with
        {
            Kind = TransactionType.Expense,
            Amount = 45.5,
            Description = "Courses",
            AccountId = accountId,
            CategoryId = "28",
        };
        Assert.Single(engine.SaveTransaction(transaction));

        var dashboard = engine.Dashboard([], Today);
        Assert.Equal(1154.5, dashboard.Balances.CurrentBalance, 3);
        Assert.Equal(45.5, dashboard.Month.Expenses, 3);
        Assert.Equal("1 154,50 €", DmxFfiMethods.FormatCurrency(1154.5));
    }

    [Fact]
    public void ValidationErrorsCarryFrenchMessages()
    {
        using var engine = DmxEngine.OpenInMemory();
        var draft = engine.AccountDraft(null) with { Name = "   " };

        var error = Assert.Throws<DmxException.Validation>(() => engine.SaveAccount(draft));
        Assert.False(string.IsNullOrWhiteSpace(error.message));
    }

    [Fact]
    public void BackupRoundTripKeepsTransactions()
    {
        using var source = DmxEngine.OpenInMemory();
        var accountId = source.SaveAccount(source.AccountDraft(null) with { Name = "Livret" });
        source.SaveTransaction(source.TransactionDraft(null, [], Today) with
        {
            Kind = TransactionType.Income,
            Amount = 100,
            Description = "Intérêts",
            AccountId = accountId,
            CategoryId = "28",
        });

        var backup = source.ExportBackup();
        using var target = DmxEngine.OpenInMemory();
        var summary = target.RestoreBackup(backup, RestoreMode.Replace);

        Assert.Equal(1u, summary.Accounts);
        Assert.Equal(1u, summary.Transactions);
        Assert.Equal(100, target.Dashboard([], Today).Balances.CurrentBalance, 3);
    }
}
