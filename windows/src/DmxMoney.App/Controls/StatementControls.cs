using DmxMoney.Interop;
using DmxMoney.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace DmxMoney.App;

/// <summary>Aperçu du CSV : une liste déroulante de rôle par colonne, puis les premières lignes.</summary>
public sealed class CsvMappingTable : ContentControl
{
    private static readonly (string Id, string Label)[] Roles =
    [
        ("ignore", "Ignorer"),
        ("date", "Date"),
        ("amount", "Montant"),
        ("description", "Description"),
        ("category", "Catégorie"),
    ];

    public static readonly DependencyProperty WizardProperty = DependencyProperty.Register(
        nameof(Wizard), typeof(object), typeof(CsvMappingTable), new PropertyMetadata(null, (sender, _) => ((CsvMappingTable)sender).Build()));

    private readonly StackPanel host = new();

    public object? Wizard
    {
        get => GetValue(WizardProperty);
        set => SetValue(WizardProperty, value);
    }

    public CsvMappingTable()
    {
        Content = new ScrollViewer
        {
            Content = host,
            HorizontalScrollMode = ScrollMode.Auto,
            HorizontalScrollBarVisibility = ScrollBarVisibility.Auto,
            VerticalScrollMode = ScrollMode.Auto,
            MaxHeight = 260,
        };
        HorizontalContentAlignment = HorizontalAlignment.Stretch;
    }

    private void Build()
    {
        host.Children.Clear();
        if (Wizard is not StatementImportViewModel wizard || wizard.Preview is not { } preview)
        {
            return;
        }
        var columns = (int)preview.ColumnCount;

        var header = new StackPanel { Orientation = Orientation.Horizontal };
        for (var column = 0; column < columns; column++)
        {
            var index = column;
            var box = new ComboBox
            {
                Width = 150,
                Margin = new Thickness(4),
                ItemsSource = Roles.Select(role => role.Label).ToList(),
                SelectedIndex = Array.FindIndex(Roles, role => role.Id == wizard.RoleOf(index)),
            };
            box.SelectionChanged += (_, _) =>
            {
                if (box.SelectedIndex >= 0)
                {
                    wizard.SetRole(index, Roles[box.SelectedIndex].Id);
                    Build();
                }
            };
            header.Children.Add(box);
        }
        host.Children.Add(header);

        foreach (var row in preview.Rows.Take(8))
        {
            var line = new StackPanel { Orientation = Orientation.Horizontal };
            for (var column = 0; column < columns; column++)
            {
                // TextBlock n'a pas de fond en WinUI : la teinte de la colonne passe par une Border.
                line.Children.Add(new Border
                {
                    Margin = new Thickness(8, 4, 8, 4),
                    Background = wizard.RoleOf(column) == "ignore" ? null : Format.Tint("#6366f1"),
                    Child = new TextBlock
                    {
                        Text = column < row.Length ? row[column] : string.Empty,
                        Width = 150,
                        FontSize = 12,
                        TextTrimming = TextTrimming.CharacterEllipsis,
                    },
                });
            }
            host.Children.Add(line);
        }
    }
}

/// <summary>Étape « compte » de l'assistant : comptes existants ou création d'un compte.</summary>
public sealed class StatementAccountPicker : ContentControl
{
    public static readonly DependencyProperty WizardProperty = DependencyProperty.Register(
        nameof(Wizard), typeof(object), typeof(StatementAccountPicker), new PropertyMetadata(null, (sender, _) => ((StatementAccountPicker)sender).Build()));

    private readonly StackPanel host = new() { Spacing = 8 };

    public object? Wizard
    {
        get => GetValue(WizardProperty);
        set => SetValue(WizardProperty, value);
    }

    public StatementAccountPicker()
    {
        Content = new ScrollViewer { Content = host, MaxHeight = 360 };
        HorizontalContentAlignment = HorizontalAlignment.Stretch;
    }

    private void Build()
    {
        host.Children.Clear();
        if (Wizard is not StatementImportViewModel wizard)
        {
            return;
        }

        foreach (var account in wizard.Accounts)
        {
            var radio = new RadioButton
            {
                GroupName = "ImportAccount",
                IsChecked = wizard.AccountChoice == account.Id,
                Content = new StackPanel
                {
                    Orientation = Orientation.Horizontal,
                    Spacing = 8,
                    Children =
                    {
                        new LucideIcon { Glyph = account.Icon, Size = 16, Stroke = Format.Brush(account.Color) },
                        new TextBlock { Text = account.Name, FontWeight = Microsoft.UI.Text.FontWeights.Medium },
                    },
                },
            };
            var id = account.Id;
            radio.Checked += (_, _) => wizard.AccountChoice = id;
            host.Children.Add(radio);
        }

        var newAccount = new RadioButton
        {
            GroupName = "ImportAccount",
            IsChecked = wizard.AccountChoice == StatementImportViewModel.NewAccountId,
            Content = new TextBlock { Text = "Nouveau compte", FontWeight = Microsoft.UI.Text.FontWeights.Medium },
        };
        newAccount.Checked += (_, _) =>
        {
            wizard.AccountChoice = StatementImportViewModel.NewAccountId;
            Build();
        };
        host.Children.Add(newAccount);

        if (wizard.AccountChoice != StatementImportViewModel.NewAccountId)
        {
            return;
        }

        var name = new TextBox { PlaceholderText = "Nom du compte", Text = wizard.NewAccountName, Margin = new Thickness(28, 0, 0, 0) };
        name.TextChanged += (_, _) => wizard.NewAccountName = name.Text;
        host.Children.Add(name);

        var type = new ComboBox
        {
            ItemsSource = wizard.AccountTypes,
            SelectedItem = wizard.NewAccountType,
            Margin = new Thickness(28, 0, 0, 0),
            MinWidth = 200,
        };
        type.SelectionChanged += (_, _) =>
        {
            if (type.SelectedItem is string selected)
            {
                wizard.NewAccountType = selected;
            }
        };
        host.Children.Add(type);

        var balance = new TextBox
        {
            PlaceholderText = "Solde final (optionnel)",
            Text = wizard.FinalBalanceText,
            Margin = new Thickness(28, 0, 0, 0),
        };
        balance.TextChanged += (_, _) => wizard.FinalBalanceText = balance.Text;
        host.Children.Add(balance);

        host.Children.Add(new TextBlock
        {
            Text = "Saisissez le solde final du relevé pour calculer automatiquement le solde initial.",
            FontSize = 11,
            Margin = new Thickness(28, 0, 0, 0),
            TextWrapping = TextWrapping.Wrap,
            Foreground = Palette.Resource("TextFillColorSecondaryBrush"),
        });
    }
}

/// <summary>Étape « catégories » : chaque libellé du fichier est associé à une catégorie.</summary>
public sealed class StatementCategoryTable : ContentControl
{
    public static readonly DependencyProperty WizardProperty = DependencyProperty.Register(
        nameof(Wizard), typeof(object), typeof(StatementCategoryTable), new PropertyMetadata(null, (sender, _) => ((StatementCategoryTable)sender).Build()));

    private readonly StackPanel host = new() { Spacing = 8 };

    public object? Wizard
    {
        get => GetValue(WizardProperty);
        set => SetValue(WizardProperty, value);
    }

    public StatementCategoryTable()
    {
        Content = new ScrollViewer { Content = host, MaxHeight = 360 };
        HorizontalContentAlignment = HorizontalAlignment.Stretch;
    }

    private void Build()
    {
        host.Children.Clear();
        if (Wizard is not StatementImportViewModel wizard)
        {
            return;
        }
        foreach (var source in wizard.Sources)
        {
            var row = new Grid { ColumnSpacing = 10 };
            row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
            row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
            row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });

            var label = new TextBlock
            {
                Text = source,
                FontSize = 13,
                FontWeight = Microsoft.UI.Text.FontWeights.Medium,
                VerticalAlignment = VerticalAlignment.Center,
                TextTrimming = TextTrimming.CharacterEllipsis,
            };
            Grid.SetColumn(label, 0);
            row.Children.Add(label);

            var arrow = new LucideIcon { Glyph = "ArrowRight", Size = 14, VerticalAlignment = VerticalAlignment.Center };
            Grid.SetColumn(arrow, 1);
            row.Children.Add(arrow);

            var options = new List<string> { $"+ Créer « {source} »" };
            options.AddRange(wizard.Categories.Select(category => category.Name));
            var mapped = wizard.CategoryMapping.TryGetValue(source, out var value) ? value : StatementImportViewModel.NewCategoryId;
            var selected = mapped == StatementImportViewModel.NewCategoryId
                ? 0
                : wizard.Categories.ToList().FindIndex(category => category.Id == mapped) + 1;

            var box = new ComboBox { ItemsSource = options, SelectedIndex = Math.Max(selected, 0), MinWidth = 220 };
            var key = source;
            box.SelectionChanged += (_, _) =>
            {
                wizard.CategoryMapping[key] = box.SelectedIndex <= 0
                    ? StatementImportViewModel.NewCategoryId
                    : wizard.Categories[box.SelectedIndex - 1].Id;
            };
            Grid.SetColumn(box, 2);
            row.Children.Add(box);
            host.Children.Add(row);
        }
    }
}
