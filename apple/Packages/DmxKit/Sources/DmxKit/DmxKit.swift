// Les apps n'importent que DmxKit : les types générés du noyau Rust sont réexportés.
@_exported import DmxCore

/// `Category` entre en conflit avec le type Objective-C du même nom sur macOS.
public typealias DmxCategory = DmxCore.Category
/// `Transaction` entre en conflit avec `SwiftUI.Transaction`.
public typealias DmxTransaction = DmxCore.Transaction
