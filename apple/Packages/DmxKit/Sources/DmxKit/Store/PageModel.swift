import Combine
import Foundation

/// Base des modèles de page : recalcule la vue du noyau quand les données ou le filtre changent.
///
/// Les sous-classes publient leurs valeurs avec `willSet { objectWillChange.send() }` plutôt que
/// `@Published` : sous macOS 10.15, `@Published` déclaré dans une sous-classe ne notifie pas SwiftUI.
open class PageModel: ObservableObject {
    public let store: AppStore

    /// Une page masquée ne se recalcule qu'au moment où elle redevient visible.
    public var isActive = true {
        didSet {
            if !isActive {
                if pendingRefresh != nil { needsRefresh = true }
                pendingRefresh?.cancel()
                pendingRefresh = nil
                refreshGeneration &+= 1
                readTask?.cancel()
                if isLoading { needsRefresh = true; isLoading = false }
            }
            if isActive && needsRefresh {
                needsRefresh = false
                refresh()
            }
        }
    }

    private var needsRefresh = false
    private var cancellable: AnyCancellable?
    private var readTask: StoreReadTask?
    private var pendingRefresh: DispatchWorkItem?
    private var refreshGeneration: UInt64 = 0
    @Published public private(set) var isLoading = false

    public init(store: AppStore) {
        self.store = store
        cancellable = store.$revision
            .dropFirst()
            .receive(on: DispatchQueue.main)
            .sink { [weak self] _ in self?.setNeedsRefresh() }
    }

    public func setNeedsRefresh(debounce: Bool = false) {
        pendingRefresh?.cancel()
        pendingRefresh = nil
        refreshGeneration &+= 1
        readTask?.cancel()
        if debounce && isActive {
            let generation = refreshGeneration
            let task = DispatchWorkItem { [weak self] in
                guard let self, self.refreshGeneration == generation else { return }
                self.setNeedsRefresh()
            }
            pendingRefresh = task
            DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(150), execute: task)
            return
        }
        if isActive {
            refresh()
        } else {
            needsRefresh = true
        }
    }

    public func load<T>(_ work: @escaping (DmxEngine) throws -> T, apply: @escaping (T) -> Void) {
        readTask?.cancel()
        isLoading = true
        readTask = store.fetch(work, completion: { [weak self] value in
            guard let self, self.isActive else { return }
            self.isLoading = false
            apply(value)
        }, failure: { [weak self] message in
            guard let self, self.isActive else { return }
            self.isLoading = false
            self.store.errorMessage = message
        })
    }

    deinit { pendingRefresh?.cancel(); readTask?.cancel() }

    /// À surcharger : relit la vue calculée auprès du noyau.
    open func refresh() {}
}
