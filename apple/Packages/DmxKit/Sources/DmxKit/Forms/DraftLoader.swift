import SwiftUI

/// Loads a stable editing baseline once, outside View.body and outside the UI thread.
public struct DraftLoader<Value, Content: View>: View {
    @EnvironmentObject private var store: AppStore
    @State private var value: Value?
    @State private var error: String?
    @State private var task: StoreReadTask?
    private let load: (DmxEngine) throws -> Value
    private let content: (Value) -> Content
    private let onClose: () -> Void

    public init(load: @escaping (DmxEngine) throws -> Value, onClose: @escaping () -> Void,
                @ViewBuilder content: @escaping (Value) -> Content) {
        self.load = load
        self.onClose = onClose
        self.content = content
    }

    public var body: some View {
        Group {
            if let value { content(value) }
            else {
                VStack(spacing: 12) {
                    Text(error ?? "Chargement du formulaire…")
                        .foregroundColor(error == nil ? .secondary : DmxColors.expense)
                        .fixedSize(horizontal: false, vertical: true)
                    Button("Annuler", action: onClose)
                }
                .padding(24)
            }
        }
        .onAppear {
            guard task == nil else { return }
            task = store.fetch(load, completion: { value = $0 }, failure: { error = $0 })
        }
        .onDisappear { task?.cancel() }
    }
}
