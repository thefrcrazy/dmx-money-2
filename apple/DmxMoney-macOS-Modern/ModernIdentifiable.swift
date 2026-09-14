import DmxCore
import DmxKit

// Les tables natives (`Table`) demandent des lignes identifiables. Les types du noyau
// portent déjà un identifiant : on déclare seulement la conformité côté app.
extension DmxCore.Category: @retroactive Identifiable {}

extension JournalRow: @retroactive Identifiable {
    public var id: String { transaction.id }
}

extension ScheduledRow: @retroactive Identifiable {
    public var id: String { scheduled.id }
}
