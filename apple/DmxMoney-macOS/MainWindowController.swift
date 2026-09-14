import AppKit
import Combine
import DmxKit
import SwiftUI

/// Fenêtre principale : barre latérale, page courante, feuilles de formulaires et alertes.
final class MainWindowController: NSWindowController, NSWindowDelegate {
    private let store: AppStore
    private let splitViewController = NSSplitViewController()
    private let sidebarController: SidebarViewController
    private let contentController: ContentViewController
    private var cancellables = Set<AnyCancellable>()
    private weak var presentedSheet: NSViewController?
    private var presentedRequestId: String?
    private var isShowingError = false

    init(store: AppStore, actions: SettingsActions) {
        self.store = store
        sidebarController = SidebarViewController(store: store)
        contentController = ContentViewController(store: store, actions: actions)

        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 1320, height: 840),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.title = "DmxMoney"
        window.minSize = NSSize(width: 1080, height: 660)
        window.isReleasedWhenClosed = false
        window.tabbingMode = .disallowed
        super.init(window: window)
        window.delegate = self

        let sidebarItem = NSSplitViewItem(viewController: sidebarController)
        sidebarItem.canCollapse = true
        sidebarItem.minimumThickness = 200
        sidebarItem.maximumThickness = 280
        let contentItem = NSSplitViewItem(viewController: contentController)
        contentItem.minimumThickness = 820
        splitViewController.addSplitViewItem(sidebarItem)
        splitViewController.addSplitViewItem(contentItem)
        splitViewController.splitView.dividerStyle = .thin
        window.contentViewController = splitViewController

        window.setContentSize(NSSize(width: 1320, height: 840))
        if !window.setFrameUsingName("DmxMoneyMainWindow") {
            window.center()
        }
        window.setFrameAutosaveName("DmxMoneyMainWindow")

        bind()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) n'est pas utilisé")
    }

    private func bind() {
        store.$route
            .removeDuplicates()
            .sink { [weak self] route in
                self?.contentController.show(route)
                self?.sidebarController.select(route)
            }
            .store(in: &cancellables)

        store.$form
            .receive(on: DispatchQueue.main)
            .sink { [weak self] request in self?.presentForm(request) }
            .store(in: &cancellables)

        store.$confirmation
            .compactMap { $0 }
            .receive(on: DispatchQueue.main)
            .sink { [weak self] request in self?.presentConfirmation(request) }
            .store(in: &cancellables)

        store.$errorMessage
            .compactMap { $0 }
            .receive(on: DispatchQueue.main)
            .sink { [weak self] message in self?.presentError(message) }
            .store(in: &cancellables)

        store.$toast
            .receive(on: DispatchQueue.main)
            .sink { [weak self] message in self?.contentController.showToast(message) }
            .store(in: &cancellables)
    }

    // MARK: - Fenêtre

    func windowShouldClose(_ sender: NSWindow) -> Bool {
        // Fermer masque la fenêtre : l'app reste dans la barre des menus et le pont reste actif.
        sender.orderOut(nil)
        return false
    }

    // MARK: - Formulaires

    private func presentForm(_ request: FormRequest?) {
        if let sheet = presentedSheet {
            if request?.id == presentedRequestId { return }
            presentedSheet = nil
            presentedRequestId = nil
            splitViewController.dismiss(sheet)
        }
        guard let request = request else { return }
        showWindow(nil)

        let root = StoreRoot(store: store) {
            FormHost(request: request, onClose: { [weak self] in
                DispatchQueue.main.async { self?.store.form = nil }
            })
        }
        let controller = NSHostingController(rootView: root)
        let width = CGFloat(request.preferredWidth)
        controller.view.setFrameSize(NSSize(width: width, height: 200))
        let fitting = controller.view.fittingSize
        controller.preferredContentSize = NSSize(width: max(width, fitting.width), height: fitting.height + 24)
        if #available(macOS 13.0, *) {
            controller.sizingOptions = [.preferredContentSize]
        }
        presentedRequestId = request.id
        presentedSheet = controller
        splitViewController.presentAsSheet(controller)
    }

    // MARK: - Alertes

    private func presentConfirmation(_ request: ConfirmRequest) {
        guard let window = window else { return }
        let alert = NSAlert()
        alert.messageText = request.title
        alert.informativeText = request.message
        alert.alertStyle = .warning
        let confirm = alert.addButton(withTitle: request.confirmTitle)
        alert.addButton(withTitle: "Annuler")
        if #available(macOS 11.0, *) {
            confirm.hasDestructiveAction = request.destructive
        }
        showWindow(nil)
        alert.beginSheetModal(for: window.attachedSheet ?? window) { [weak self] response in
            self?.store.confirmation = nil
            if response == .alertFirstButtonReturn {
                request.action()
            }
        }
    }

    private func presentError(_ message: String) {
        guard !isShowingError, let window = window else {
            store.errorMessage = nil
            return
        }
        isShowingError = true
        let alert = NSAlert()
        alert.messageText = "DmxMoney"
        alert.informativeText = message
        alert.alertStyle = .warning
        alert.addButton(withTitle: "OK")
        let host = window.attachedSheet ?? window
        if host.isVisible {
            alert.beginSheetModal(for: host) { [weak self] _ in
                self?.isShowingError = false
                self?.store.errorMessage = nil
            }
        } else {
            alert.runModal()
            isShowingError = false
            store.errorMessage = nil
        }
    }
}
