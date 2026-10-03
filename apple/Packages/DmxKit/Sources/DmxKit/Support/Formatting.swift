import Foundation

/// Résultats exacts du noyau, conservés en mémoire avec une éviction FIFO bornée.
final class BoundedFormatCache<Key: Hashable>: @unchecked Sendable {
    private let capacity: Int
    private let lock = NSLock()
    private var values: [Key: String] = [:]
    private var keys: [Key] = []
    private var nextEviction = 0

    init(capacity: Int) {
        precondition(capacity > 0)
        self.capacity = capacity
    }

    func value(for key: Key, create: () -> String) -> String {
        lock.lock()
        defer { lock.unlock() }
        if let value = values[key] { return value }
        let value = create()
        if keys.count == capacity {
            values.removeValue(forKey: keys[nextEviction])
            keys[nextEviction] = key
            nextEviction = (nextEviction + 1) % capacity
        } else {
            keys.append(key)
        }
        values[key] = value
        return value
    }
}

/// Formats fr-FR fournis par le noyau (espaces fines, abréviations de mois de 1.x).
public enum Money {
    private static let currency = BoundedFormatCache<UInt64>(capacity: 2048)
    private static let roundedCurrency = BoundedFormatCache<UInt64>(capacity: 256)

    public static func format(_ amount: Double) -> String {
        // La représentation binaire distingue notamment +0 et -0 sans modifier l'arrondi.
        currency.value(for: amount.bitPattern) { formatCurrency(amount: amount) }
    }

    /// « 1 235 € » : montant arrondi à l'euro, comme les cartes de la vue d'ensemble.
    public static func rounded(_ amount: Double) -> String {
        roundedCurrency.value(for: amount.bitPattern) { formatCurrencyRounded(amount: amount) }
    }

    /// Montant signé selon le type : « +12,00 € » pour un revenu, « -12,00 € » pour une dépense.
    /// Montant d'une ligne : un virement garde sa couleur, mais prend le signe de sa jambe telle
    /// qu'elle est stockée en base (sortie en dépense « − », entrée en revenu « + »).
    public static func signed(_ amount: Double, display: TransactionType, stored: TransactionType) -> String {
        guard display == .transfer else { return signed(amount, kind: display) }
        return stored == .income ? "+" + format(amount) : "-" + format(amount)
    }

    public static func signed(_ amount: Double, kind: TransactionType, showMinus: Bool = true) -> String {
        switch kind {
        case .income:
            return "+" + format(amount)
        case .expense:
            return showMinus ? "-" + format(amount) : format(amount)
        case .transfer:
            return format(amount)
        }
    }
}

public enum DayFormat {
    private static let numericDates = BoundedFormatCache<String>(capacity: 512)
    private static let shortDates = BoundedFormatCache<String>(capacity: 512)
    private static let mediumDates = BoundedFormatCache<String>(capacity: 512)
    private static let longDates = BoundedFormatCache<String>(capacity: 512)

    /// 14/09/2026
    public static func numeric(_ date: String) -> String {
        numericDates.value(for: date) { formatDateNumeric(date: date) }
    }
    /// 14 sept.
    public static func short(_ date: String) -> String {
        shortDates.value(for: date) { formatDateShort(date: date) }
    }
    /// 14 sept. 2026
    public static func medium(_ date: String) -> String {
        mediumDates.value(for: date) { formatDateMedium(date: date) }
    }
    /// lundi 14 septembre 2026
    public static func long(_ date: String) -> String {
        longDates.value(for: date) { formatDateLong(date: date) }
    }

    /// « Aujourd'hui », « Demain », « Dans 3 jours », « En retard de 2 jours ».
    public static func relative(days: Int64) -> String {
        switch days {
        case 0: return "Aujourd'hui"
        case 1: return "Demain"
        case let value where value < 0: return value == -1 ? "En retard d'un jour" : "En retard de \(-value) jours"
        default: return "Dans \(days) jours"
        }
    }
}

/// Conversion entre les dates `YYYY-MM-DD` du noyau et `Date` pour les sélecteurs natifs.
public enum DayString {
    private static let formatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.calendar = Calendar(identifier: .gregorian)
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.timeZone = TimeZone.current
        formatter.dateFormat = "yyyy-MM-dd"
        return formatter
    }()

    public static func date(_ value: String) -> Date {
        formatter.date(from: String(value.prefix(10))) ?? Date()
    }

    public static func string(_ date: Date) -> String {
        formatter.string(from: date)
    }
}

/// Saisie des montants : virgule ou point, espaces ignorés (analyse faite par le noyau).
public enum AmountInput {
    private static let formatter: NumberFormatter = {
        let formatter = NumberFormatter()
        formatter.locale = Locale(identifier: "fr_FR")
        formatter.numberStyle = .decimal
        formatter.usesGroupingSeparator = false
        formatter.minimumFractionDigits = 0
        formatter.maximumFractionDigits = 2
        return formatter
    }()

    public static func text(_ value: Double, emptyWhenZero: Bool = true) -> String {
        if emptyWhenZero && value == 0 { return "" }
        return formatter.string(from: NSNumber(value: value)) ?? ""
    }

    public static func parse(_ text: String) -> Double? {
        parseAmountInput(value: text)
    }
}

extension Array {
    /// Découpe en lignes de `size` éléments (grilles compatibles macOS 10.15, sans LazyVGrid).
    public func chunked(_ size: Int) -> [[Element]] {
        guard size > 0 else { return [self] }
        return stride(from: 0, to: count, by: size).map { Array(self[$0..<Swift.min($0 + size, count)]) }
    }
}
