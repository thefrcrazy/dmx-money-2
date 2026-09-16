#!/usr/bin/env python3
"""Exercise the actual AppKit download flow without installing or restarting DmxMoney.

Requires a logged-in macOS desktop and Xcode. Uses a loopback HTTP fixture and
an isolated temporary directory; never opens the user's database.
"""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import os
import subprocess
import tempfile
import threading
import time

ROOT = Path(__file__).resolve().parents[2]


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == '/cancel':
            time.sleep(0.5)
        self.send_response(404 if self.path == '/error' else 200)
        self.end_headers()
        try:
            self.wfile.write(b'fixture-dmg')
        except BrokenPipeError:
            pass

    def log_message(self, *_):
        pass


source = (ROOT / 'apple/DmxMoney-macOS-Shared/UpdateChecker.swift').read_text()
source = source.replace('import DmxKit', '''enum AppInfo {
    static let version = "1.0"
    static let systemVersion = "26.0"
    static func isVersion(_ version: String, newerThan other: String) -> Bool { true }
}''')
a = source.index('    private func applyDmgAndRestart(')
b = source.index('    private func presentUpToDate', a)
source = source[:a] + '''    private func applyDmgAndRestart(dmgURL: URL) {
        guard CommandLine.arguments[1] == "success",
              (try? String(contentsOf: dmgURL, encoding: .utf8)) == "fixture-dmg" else { exit(2) }
        try? FileManager.default.removeItem(at: dmgURL)
        print("PASS: completed download survives the URLSession callback and closes its sheet")
        exit(0)
    }
''' + source[b:]
source += '''
extension UpdateChecker {
    func testDownload(_ url: URL) {
        performDownloadAndRestart(build: Feed.Build(url: url, minimumSystemVersion: nil), version: "test")
    }
}
let app = NSApplication.shared
app.setActivationPolicy(.accessory)
app.finishLaunching()
let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 200), styleMask: [.titled], backing: .buffered, defer: false)
window.title = "DmxMoney updater regression test"
window.makeKeyAndOrderFront(nil)
window.makeMain()
window.makeKey()
app.activate(ignoringOtherApps: true)
let mode = CommandLine.arguments[1]
DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
    guard NSApp.keyWindow != nil || NSApp.mainWindow != nil else { exit(5) }
    UpdateChecker.shared.testDownload(URL(string: CommandLine.arguments[2])!)
    guard let downloading = window.attachedSheet else { exit(3) }
    if mode == "cancel" { window.endSheet(downloading, returnCode: .alertFirstButtonReturn) }
    DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) {
        if mode == "cancel" && window.attachedSheet == nil {
            print("PASS: cancellation closes the sheet and late completion does not reopen it")
            exit(0)
        }
        if mode == "error", let failure = window.attachedSheet, failure !== downloading {
            print("PASS: HTTP failure replaces the download sheet with an error")
            exit(0)
        }
        exit(4)
    }
}
app.run()
'''
server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
threading.Thread(target=server.serve_forever, daemon=True).start()
try:
    with tempfile.TemporaryDirectory(prefix='dmx-updater-test-') as directory:
        folder = Path(directory)
        swift = folder / 'main.swift'
        swift.write_text(source)
        binary = folder / 'test-updater'
        env = dict(os.environ)
        env.setdefault('DEVELOPER_DIR', '/Applications/Xcode.app/Contents/Developer')
        subprocess.run(['xcrun', 'swiftc', '-module-cache-path', str(folder / 'cache'), str(swift), '-o', str(binary)], env=env, check=True)
        for mode in ('success', 'error', 'cancel'):
            subprocess.run([str(binary), mode, f'http://127.0.0.1:{server.server_port}/{mode}'], env=env, check=True, timeout=15)
finally:
    server.shutdown()
