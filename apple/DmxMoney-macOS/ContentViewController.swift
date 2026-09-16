import AppKit
import DmxKit
import SwiftUI

/// Zone principale : barre de filtre et de soldes, puis la page courante (conservée entre deux visites).
final class ContentViewController: NSViewController {
    private let store: AppStore
    private let actions: SettingsActions
    private let pageContainer = NSView()
    private var controllers: [AppRoute: NSViewController] = [:]
    private var models: [AppRoute: PageModel] = [:]
    private weak var currentController: NSViewController?
    private var currentRoute: AppRoute?
    private var toastView: NSView?

    init(store: AppStore, actions: SettingsActions) {
        self.store = store
        self.actions = actions
        super.init(nibName: nil, bundle: nil)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) n'est pas utilisé")
    }

    override func loadView() {
        let root = NSView()
        let header = NSHostingView(rootView: StoreRoot(store: store) { HeaderBar() })
        header.translatesAutoresizingMaskIntoConstraints = false
        let separator = NSBox()
        separator.boxType = .separator
        separator.translatesAutoresizingMaskIntoConstraints = false
        pageContainer.translatesAutoresizingMaskIntoConstraints = false

        root.addSubview(header)
        root.addSubview(separator)
        root.addSubview(pageContainer)
        NSLayoutConstraint.activate([
            header.topAnchor.constraint(equalTo: root.topAnchor),
            header.leadingAnchor.constraint(equalTo: root.leadingAnchor),
            header.trailingAnchor.constraint(equalTo: root.trailingAnchor),
            header.heightAnchor.constraint(equalToConstant: 62),
            separator.topAnchor.constraint(equalTo: header.bottomAnchor),
            separator.leadingAnchor.constraint(equalTo: root.leadingAnchor),
            separator.trailingAnchor.constraint(equalTo: root.trailingAnchor),
            pageContainer.topAnchor.constraint(equalTo: separator.bottomAnchor),
            pageContainer.leadingAnchor.constraint(equalTo: root.leadingAnchor),
            pageContainer.trailingAnchor.constraint(equalTo: root.trailingAnchor),
            pageContainer.bottomAnchor.constraint(equalTo: root.bottomAnchor),
        ])
        view = root
    }

    func show(_ route: AppRoute) {
        _ = view
        guard route != currentRoute else { return }
        if let previous = currentRoute {
            models[previous]?.isActive = false
        }
        currentRoute = route
        let controller = self.controller(for: route)
        models[route]?.isActive = true

        if let current = currentController, current !== controller {
            current.view.removeFromSuperview()
            current.removeFromParent()
        }
        addChild(controller)
        controller.view.translatesAutoresizingMaskIntoConstraints = false
        pageContainer.addSubview(controller.view)
        NSLayoutConstraint.activate([
            controller.view.topAnchor.constraint(equalTo: pageContainer.topAnchor),
            controller.view.leadingAnchor.constraint(equalTo: pageContainer.leadingAnchor),
            controller.view.trailingAnchor.constraint(equalTo: pageContainer.trailingAnchor),
            controller.view.bottomAnchor.constraint(equalTo: pageContainer.bottomAnchor),
        ])
        currentController = controller
    }

    private func controller(for route: AppRoute) -> NSViewController {
        if let existing = controllers[route] {
            return existing
        }
        let controller: NSViewController
        switch route {
        case .dashboard:
            let model = DashboardModel(store: store)
            models[route] = model
            controller = host(DashboardPage(model: model))
        case .accounts:
            let model = AccountsModel(store: store)
            models[route] = model
            controller = host(AccountsPage(model: model))
        case .transactions:
            let model = JournalModel(store: store)
            models[route] = model
            controller = JournalViewController(store: store, model: model)
        case .budget:
            let model = BudgetModel(store: store)
            models[route] = model
            controller = host(BudgetPage(model: model))
        case .scheduled:
            let model = ScheduledModel(store: store)
            models[route] = model
            controller = host(ScheduledPage(model: model))
        case .analytics:
            let model = AnalyticsModel(store: store)
            models[route] = model
            controller = host(AnalyticsPage(model: model))
        case .predictions:
            let model = PredictionsModel(store: store)
            models[route] = model
            controller = host(PredictionsPage(model: model))
        case .categories:
            controller = host(CategoriesPage())
        case .settings:
            controller = host(SettingsPage(actions: actions))
        }
        controllers[route] = controller
        return controller
    }

    private func host<Page: View>(_ page: Page) -> NSViewController {
        NSHostingController(rootView: StoreRoot(store: store) { page })
    }

    // MARK: - Messages

    func showToast(_ message: String?) {
        _ = view
        toastView?.removeFromSuperview()
        toastView = nil
        guard let message = message else { return }
        let toast = NSHostingView(rootView: ToastView(message))
        toast.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(toast, positioned: .above, relativeTo: nil)
        NSLayoutConstraint.activate([
            toast.centerXAnchor.constraint(equalTo: view.centerXAnchor),
            toast.bottomAnchor.constraint(equalTo: view.bottomAnchor, constant: -28),
        ])
        toastView = toast
    }
}

/// Barre supérieure : filtre global de comptes et soldes Pointé / Actuel.
struct HeaderBar: View {
    @EnvironmentObject private var store: AppStore

    var body: some View {
        HStack(spacing: 14) {
            if store.route.usesAccountFilter || !store.selectedAccountIds.isEmpty {
                Text("Compte :").font(.system(size: 12, weight: .medium)).foregroundColor(.secondary)
                AccountFilterButton()
            } else {
                DmxIcon(store.route.icon, size: 15).foregroundColor(.accentColor)
                Text(store.route.title).font(.system(size: 14, weight: .semibold))
            }
            Spacer()
            if store.route.showsBalances {
                balance("Pointé", store.balances.checkedBalance, color: DmxColors.income, size: 16)
                Rectangle().fill(DmxPalette.separator).frame(width: 1, height: 30)
                balance("Actuel", store.balances.currentBalance, color: .primary, size: 19)
            }
        }
        .padding(.horizontal, 24)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(DmxPalette.pageBackground)
    }

    private func balance(_ label: String, _ value: Double, color: Color, size: CGFloat) -> some View {
        VStack(alignment: .trailing, spacing: 1) {
            SectionLabel(label)
            Text(Money.format(value)).font(.system(size: size, weight: .bold)).foregroundColor(color)
        }
    }
}
