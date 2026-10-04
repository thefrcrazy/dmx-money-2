import AppKit
import DmxKit
import QuartzCore
import SwiftUI

final class Scenario: ObservableObject { @Published var journal = true }
struct Root: View {
    @ObservedObject var state: Scenario
    let model: JournalModel
    let store: AppStore
    var body: some View {
        if state.journal {
            ModernJournal(model: model).environmentObject(store)
        } else {
            Text("Budget fictif").frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }
}
func findTable(_ view: NSView) -> NSTableView? {
    if let table = view as? NSTableView { return table }
    for child in view.subviews { if let table = findTable(child) { return table } }
    return nil
}
func pump(_ seconds: Double) {
    let end = Date(timeIntervalSinceNow: seconds)
    while Date() < end { _ = RunLoop.current.run(mode: .default, before: end) }
}
func countViews(_ view: NSView) -> Int { 1 + view.subviews.reduce(0) { $0 + countViews($1) } }
let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let scenario = Scenario()
let model = JournalModel(count: 30000)
let store = AppStore()
let window = NSWindow(
    contentRect: NSRect(x: 100, y: 100, width: 1280, height: 800), styleMask: [.titled, .closable, .resizable],
    backing: .buffered, defer: false)
window.title = "DmxMoney — fixture de performance"
let host = NSHostingView(rootView: Root(state: scenario, model: model, store: store))
window.contentView = host
window.makeKeyAndOrderFront(nil)
let scenarioTimer = Timer.scheduledTimer(withTimeInterval: 2, repeats: false) { _ in
    guard let table = findTable(host) else {
        print("NO TABLE")
        exit(2)
    }
    var times: [Double] = []
    for i in 0..<80 {
        let start = DispatchTime.now().uptimeNanoseconds
        table.scrollRowToVisible(i % 2 == 0 ? 29990 - i : i)
        host.layoutSubtreeIfNeeded()
        window.displayIfNeeded()
        CATransaction.flush()
        _ = RunLoop.current.run(mode: .default, before: Date(timeIntervalSinceNow: 0.004))
        times.append(Double(DispatchTime.now().uptimeNanoseconds - start) / 1e6)
    }
    let viewCount = countViews(host)
    if let bitmap = host.bitmapImageRepForCachingDisplay(in: host.bounds) {
        host.cacheDisplay(in: host.bounds, to: bitmap)
        try! bitmap.representation(using: .png, properties: [:])!.write(
            to: URL(fileURLWithPath: CommandLine.arguments[1] + ".png"))
    }
    // Functional assertions only apply to the new controller; the baseline has SwiftUI delegates.
    var assertions = 0
    // BEGIN CONTROLLER ASSERTIONS
    if let controller = table.delegate as? JournalViewController {
        table.scrollRowToVisible(10)
        host.layoutSubtreeIfNeeded()
        window.displayIfNeeded()
        let cell = table.view(atColumn: 3, row: 10, makeIfNecessary: true) as! EditableCell
        precondition(controller.control(cell.field, textShouldBeginEditing: NSTextView()))
        cell.field.stringValue = "Description fictive modifiée"
        controller.controlTextDidEndEditing(Notification(name: NSText.didEndEditingNotification, object: cell.field))
        pump(0.02)
        precondition(model.actions.contains("description|fiction-10|Description fictive modifiée"))
        assertions += 1
        let amountCell = table.view(atColumn: 4, row: 10, makeIfNecessary: true) as! EditableCell
        precondition(controller.control(amountCell.field, textShouldBeginEditing: NSTextView()))
        amountCell.field.stringValue = "12.50"
        controller.controlTextDidEndEditing(
            Notification(name: NSText.didEndEditingNotification, object: amountCell.field))
        pump(0.03)
        precondition(model.actions.contains("amount|fiction-10|12.50"))
        assertions += 1
        precondition(controller.control(cell.field, textShouldBeginEditing: NSTextView()))
        model.rowsRevision += 1
        cell.field.stringValue = "Brouillon devenu périmé"
        controller.controlTextDidEndEditing(Notification(name: NSText.didEndEditingNotification, object: cell.field))
        precondition(!model.actions.contains { $0.contains("périmé") })
        assertions += 1
        pump(0.03)
        table.sortDescriptors = [NSSortDescriptor(key: "amount", ascending: false)]
        pump(0.15)
        let prepared = JournalTablePreparation.prepare(model.rows, order: [JournalSort(key: "amount", ascending: true)])
        precondition(prepared.rows.first?.transaction.amount == 0.37)
        precondition(prepared.rows[1].id == "fiction-500")
        assertions += 2
        precondition(prepared.indexes["fiction-10"] != nil)
        assertions += 1
        model.selection = ["fiction-10", "fiction-25000"]
        pump(0.05)
        precondition(table.selectedRowIndexes.count == 2)
        assertions += 1
        let before = model.actions.count
        table.scrollRowToVisible(0)
        host.layoutSubtreeIfNeeded()
        window.displayIfNeeded()
        let status = table.view(atColumn: 6, row: 0, makeIfNecessary: true) as! StatusCell
        (status.subviews.first as! NSButton).performClick(nil)
        precondition(model.actions.count == before + 1)
        assertions += 1
        precondition(model.actions.last == "check|fiction-499")
        assertions += 1
        let shown = table.view(atColumn: 3, row: 0, makeIfNecessary: true) as! EditableCell
        precondition(shown.field.stringValue == "Opération fictive 499")
        assertions += 1
    }
    // END CONTROLLER ASSERTIONS
    let start = DispatchTime.now().uptimeNanoseconds
    scenario.journal = false
    while findTable(host) != nil && Double(DispatchTime.now().uptimeNanoseconds - start) / 1e9 < 10 {
        _ = RunLoop.current.run(mode: .default, before: Date(timeIntervalSinceNow: 0.002))
        host.layoutSubtreeIfNeeded()
        window.displayIfNeeded()
    }
    let switchMs = Double(DispatchTime.now().uptimeNanoseconds - start) / 1e6
    if assertions > 0 {
        pump(0.05)
        precondition(table.delegate == nil && table.dataSource == nil)
        assertions += 1
    }
    let sorted = times.sorted()
    let report: [String: Any] = [
        "rows": 30000, "scrollIterations": 80, "sumScrollMs": times.reduce(0, +), "p50ScrollMs": sorted[40],
        "p95ScrollMs": sorted[76], "maxScrollMs": sorted.last!, "switchMs": switchMs, "viewCount": viewCount,
        "functionalAssertions": assertions, "tableDetached": findTable(host) == nil,
        "scope":
            "actual ModernJournal source, synthetic DmxKit collaborators; scrollRowToVisible + layout/display flush; destination lightweight fixture; not physical gesture FPS",
    ]
    let data = try! JSONSerialization.data(withJSONObject: report, options: [.prettyPrinted, .sortedKeys])
    print(String(decoding: data, as: UTF8.self))
    window.orderOut(nil)
    app.terminate(nil)
}
app.run()
