using System.ComponentModel;
using CommunityToolkit.Mvvm.ComponentModel;

namespace DmxMoney.ViewModels;

/// <summary>
/// Base des pages : recalcule la vue du noyau quand les données ou le filtre changent.
/// Une page masquée ne se recalcule qu'au moment où elle redevient visible.
/// </summary>
public abstract class PageViewModel : ObservableObject, IDisposable
{
    private bool needsRefresh;
    private bool isActive = true;

    public EngineStore Store { get; }

    public bool IsActive
    {
        get => isActive;
        set
        {
            isActive = value;
            if (value && needsRefresh)
            {
                needsRefresh = false;
                Refresh();
            }
        }
    }

    protected PageViewModel(EngineStore store)
    {
        Store = store;
        Store.PropertyChanged += OnStoreChanged;
    }

    private void OnStoreChanged(object? sender, PropertyChangedEventArgs args)
    {
        if (args.PropertyName == nameof(EngineStore.Revision))
        {
            SetNeedsRefresh();
        }
    }

    public void SetNeedsRefresh()
    {
        if (IsActive)
        {
            Refresh();
        }
        else
        {
            needsRefresh = true;
        }
    }

    /// <summary>Relit la vue calculée auprès du noyau.</summary>
    public abstract void Refresh();

    public virtual void Dispose() => Store.PropertyChanged -= OnStoreChanged;
}
