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
            if isActive && needsRefresh {
                needsRefresh = false
                refresh()
            }
        }
    }

    private var needsRefresh = false
    private var cancellable: AnyCancellable?

    public init(store: AppStore) {
        self.store = store
        cancellable = store.$revision
            .dropFirst()
            .receive(on: DispatchQueue.main)
            .sink { [weak self] _ in self?.setNeedsRefresh() }
    }

    public func setNeedsRefresh() {
        if isActive {
            refresh()
        } else {
            needsRefresh = true
        }
    }

    /// À surcharger : relit la vue calculée auprès du noyau.
    open func refresh() {}
}
