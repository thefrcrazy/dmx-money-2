using System.ComponentModel;
using DmxMoney.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;

namespace DmxMoney.App;

public sealed partial class CategoriesPage : Page
{
    public CategoriesViewModel ViewModel { get; private set; } = null!;

    public CategoriesPage()
    {
        InitializeComponent();
        NavigationCacheMode = NavigationCacheMode.Required;
    }

    /// <summary>La catégorie « Virement » est réservée aux virements : ni modifiable, ni supprimable.</summary>
    public static bool IsLocked(string id) => id == "transfer";

    protected override void OnNavigatedTo(NavigationEventArgs args)
    {
        var shell = (ShellViewModel)args.Parameter;
        if (!ReferenceEquals(ViewModel, shell.CategoriesPage))
        {
            if (ViewModel is not null)
            {
                ViewModel.PropertyChanged -= OnViewModelChanged;
            }
            ViewModel = shell.CategoriesPage;
            ViewModel.PropertyChanged += OnViewModelChanged;
        }
        Bindings.Update();
        UpdateVisuals();
    }

    private void OnViewModelChanged(object? sender, PropertyChangedEventArgs args) => UpdateVisuals();

    private void UpdateVisuals()
    {
        CategoryGrid.ItemsSource = ViewModel.Visible;
        EmptyState.Visibility = ViewModel.Visible.Count == 0 ? Visibility.Visible : Visibility.Collapsed;
    }

    private void OnEdit(object sender, RoutedEventArgs args)
    {
        if (sender is FrameworkElement { Tag: string id })
        {
            ViewModel.EditCommand.Execute(id);
        }
    }

    private void OnDelete(object sender, RoutedEventArgs args)
    {
        if (sender is FrameworkElement { Tag: string id })
        {
            ViewModel.DeleteCommand.Execute(id);
        }
    }
}
