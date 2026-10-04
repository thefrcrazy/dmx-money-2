import DmxKit
import SwiftUI

struct FilterBar<L: View, R: View>: View {
    let leading: L
    let trailing: R
    init(@ViewBuilder _ leading: () -> L, @ViewBuilder trailing: () -> R) {
        self.leading = leading()
        self.trailing = trailing()
    }
    var body: some View {
        HStack {
            leading
            Spacer()
            trailing
        }.padding(8)
    }
}
struct FilterSelector: View {
    let title: String
    let systemImage: String
    let options: [SelectOption]
    @Binding var selection: [String]
    var body: some View { Text(title) }
}
extension Color { init(hex: String?, fallback: Color) { self = fallback } }
enum Symbols { static func name(for value: String) -> String { "wallet.bifold" } }

struct JournalHeader: View {
    let model: JournalModel
    var body: some View { Text("En-tête fictif") }
}
struct JournalEmptyState: View {
    let model: JournalModel
    var body: some View { Text("Journal vide fictif") }
}
