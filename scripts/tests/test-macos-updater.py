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
import textwrap

ROOT = Path(__file__).resolve().parents[2]

def test_installer_rollback():
    """Run the production shell with fake tools, applications and mount points only."""
    original = (ROOT / 'apple/DmxMoney-macOS-Shared/UpdateChecker.swift').read_text()
    script = original.split('static let installerScript = """', 1)[1].split('"""', 1)[0]
    script = textwrap.dedent(script).replace('\\"', '"')
    with tempfile.TemporaryDirectory(prefix='dmx-installer-rollback-') as directory:
        folder = Path(directory)
        tools = folder / 'tools'
        tools.mkdir()
        stubs = {
            'hdiutil': '#!/bin/sh\n[ "$1" = detach ] && exit 0\nshift\nwhile [ "$1" != -mountpoint ]; do shift; done\nshift\nmkdir -p "$1/DmxMoney.app"\necho new > "$1/DmxMoney.app/marker"\n',
            'PlistBuddy': '#!/bin/sh\necho com.dmxmoney.app\n',
            'codesign': '#!/bin/sh\n[ "$DMX_TEST_CASE" = signature-failure ] && exit 1\nexit 0\n',
            'ditto': '#!/bin/sh\n/bin/cp -R "$1" "$2"\n',
            'open': '#!/bin/sh\ncase "$DMX_TEST_CASE" in launch-failure|restore-failure) exit 1;; esac\n[ "$DMX_TEST_CASE" = ready-timeout ] && exit 0\nwhile [ "$#" -gt 0 ]; do\n if [ "$1" = --dmx-update-ready ]; then shift; echo ready > "$1"; fi\n shift\ndone\nexit 0\n',
            'osascript': '#!/bin/sh\nexit 0\n',
            'sleep': '#!/bin/sh\nexit 0\n',
            'mv': '#!/bin/sh\ncase "$DMX_TEST_CASE:$1" in replacement-failure:*/replacement.app|restore-failure:*/previous.app) exit 1;; esac\n/bin/mv "$@"\n',
        }
        for name, body in stubs.items():
            path = tools / name
            path.write_text(body)
            path.chmod(0o755)
        for name in ('hdiutil', 'codesign', 'ditto', 'open', 'osascript'):
            script = script.replace('/usr/bin/' + name, str(tools / name))
        script = script.replace('/usr/libexec/PlistBuddy', str(tools / 'PlistBuddy'))
        for case in ('success', 'signature-failure', 'replacement-failure', 'launch-failure', 'ready-timeout', 'restore-failure'):
            case_folder = folder / case
            case_folder.mkdir()
            app = case_folder / 'DmxMoney.app'
            app.mkdir()
            (app / 'marker').write_text('old')
            dmg = case_folder / 'fixture.dmg'
            dmg.write_text('fake')
            runner = case_folder / 'installer.sh'
            runner.write_text(script)
            env = dict(os.environ, PATH=str(tools) + ':' + os.environ['PATH'], DMX_TEST_CASE=case)
            result = subprocess.run(['/bin/sh', str(runner), '2147483647', str(dmg), str(app), 'ABCDEFGHIJ'], env=env, timeout=3, capture_output=True, text=True)
            backups = list(case_folder.glob('DmxMoney.app.update.*/previous.app/marker'))
            if case == 'success':
                assert result.returncode == 0 and (app / 'marker').read_text().strip() == 'new', result.stderr
                assert not backups
            elif case == 'restore-failure':
                assert result.returncode != 0 and len(backups) == 1 and backups[0].read_text().strip() == 'old', result.stderr
            else:
                assert result.returncode != 0 and (app / 'marker').read_text().strip() == 'old', result.stderr
                assert not backups
        print('PASS: backup restored after copy/swap/launch/readiness failures; unrecoverable backup is retained')


def test_entitlement_selection():
    script = (ROOT / 'scripts/build-macos.sh').read_text()
    resolver = script.split("<<'PY_ENTITLEMENTS'\n", 1)[1].split('\nPY_ENTITLEMENTS', 1)[0]
    import json, plistlib
    with tempfile.TemporaryDirectory(prefix='dmx-entitlement-fixture-') as directory:
        folder = Path(directory)
        app = folder / 'Fixture.app'
        (app / 'Contents').mkdir(parents=True)
        info = app / 'Contents/Info.plist'
        output = folder / 'resolved.plist'
        settings = folder / 'settings.json'
        for suffix, container, identity, accepted in [('', '', '-', True), ('-iCloud', 'iCloud.example.fixture', 'Apple Development fixture', True), ('-iCloud', 'iCloud.example.fixture', '-', False)]:
            info.write_bytes(plistlib.dumps({'DmxICloudContainer': container}))
            settings.write_text(json.dumps([{'buildSettings': {'CODE_SIGN_ENTITLEMENTS': 'DmxMoney-macOS/DmxMoney$(DMX_ENTITLEMENTS_SUFFIX).entitlements', 'DMX_ENTITLEMENTS_SUFFIX': suffix, 'DMX_ICLOUD_CONTAINER': container}}]))
            result = subprocess.run(['python3', '-c', resolver, str(settings), str(ROOT / 'apple'), str(output), str(app), identity], capture_output=True, text=True)
            assert (result.returncode == 0) == accepted, result.stderr
            if accepted:
                rights = plistlib.loads(output.read_bytes())
                assert rights.get('com.apple.developer.icloud-container-identifiers', []) == ([container] if container else [])
                assert plistlib.loads(info.read_bytes())['DmxICloudContainer'] == container
        print('PASS: effective entitlements selected and expanded; ad hoc iCloud configuration rejected')


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
    static var version = "2.0.9"
    static var systemVersion = "26.0"
    STATIC_VERSION_COMPARISON
}
let fixtureSuite = "DmxMoneyUpdaterFixture.\\(UUID().uuidString)"
let fixtureDefaults = UserDefaults(suiteName: fixtureSuite)!
var offeredVersion: String?
''')
version_source = (ROOT / 'apple/Packages/DmxKit/Sources/DmxKit/Forms/DataForms.swift').read_text()
version_start = version_source.index('    public static func isVersion(')
version_end = version_source.index('\n}\n', version_start)
version_method = version_source[version_start:version_end].replace('public static func', 'static func')
source = source.replace('    STATIC_VERSION_COMPARISON', version_method)
source = source.replace('UserDefaults.standard', 'fixtureDefaults')
a = source.index('    private func presentUpdate(')
b = source.index('    private func performDownloadAndRestart(', a)
source = source[:a] + '''    private func presentUpdate(_ feed: Feed, build: Feed.Build, current: String, silent: Bool) {
        offeredVersion = feed.version
    }

''' + source[b:]
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
source += r'''
extension UpdateChecker {
    func testReleasePolicy() {
        defer { fixtureDefaults.removePersistentDomain(forName: fixtureSuite) }
        func offer(_ version: String) -> String? {
            offeredVersion = nil
            handle(Feed(version: version, notes: nil, platforms: nil,
                        url: URL(string: "https://github.com/thefrcrazy/dmx-money-2/releases/download/v2.1.0/DmxMoney-2.1.0-apple-silicon.dmg"),
                        minimumSystemVersion: nil), silent: true)
            return offeredVersion
        }
        fixtureDefaults.removeObject(forKey: "DmxIncludePrereleases")
        AppInfo.version = "2.0.9"
        precondition(!includePrereleases, "stable installations must default to stable updates")
        precondition(offer("2.1.0-rc.1") == nil, "a direct JSON feed must honor prerelease opt-in")
        precondition(offer("2.1.0") == "2.1.0", "stable updates must remain available")
        precondition(!allowsVersion("2.1.0", prerelease: true), "the GitHub prerelease flag must also be honored")
        precondition(allowsVersion("2.1.0+build-debug"), "build metadata is not a prerelease")
        includePrereleases = true
        precondition(offer("2.1.0-rc.1") == "2.1.0-rc.1", "explicit opt-in allows RC updates")
        AppInfo.version = "2.1.0-rc.1"
        includePrereleases = false
        precondition(offer("2.1.0-rc.2") == nil, "explicit opt-out must override the installed RC")
        fixtureDefaults.removeObject(forKey: "DmxIncludePrereleases")
        precondition(includePrereleases, "RC installations must default to RC updates")
        precondition(offer("2.1.0-rc.2") == "2.1.0-rc.2")
        precondition(offer("2.1.0") == "2.1.0", "an RC can update to the final stable release")
        func release(_ version: String, available: Bool, draft: Bool = false) -> GitHubRelease {
            let suffix = Self.platform == "darwin-arm64" ? "apple-silicon" : "intel-catalina"
            let asset = GitHubRelease.Asset(name: "DmxMoney-\(version)-\(suffix).dmg", browser_download_url: URL(string: "https://github.com/thefrcrazy/dmx-money-2/releases/download/v\(version)/DmxMoney-\(version)-\(suffix).dmg")!)
            return GitHubRelease(tag_name: "v" + version, name: nil, body: nil, draft: draft, prerelease: false, assets: available ? [asset] : [])
        }
        precondition(compatibleFeed([release("2.2.0", available: false), release("2.1.0", available: true)])?.version == "2.1.0", "a release without this architecture must not hide a compatible release")
        precondition(compatibleFeed([release("2.1.0", available: true), release("2.2.0", available: true)])?.version == "2.2.0", "GitHub list order is not SemVer order")
        precondition(compatibleFeed([release("2.2.0", available: true, draft: true), release("2.1.0", available: true)])?.version == "2.1.0")
        AppInfo.systemVersion = "10.15"
        if Self.platform == "darwin-arm64" { precondition(compatibleFeed([release("2.1.0", available: true)]) == nil) }
        precondition(Self.isTrustedFeed(URL(string: "https://api.github.com/repos/thefrcrazy/dmx-money-2/releases")!))
        for value in ["http://api.github.com/repos/thefrcrazy/dmx-money-2/releases", "https://api.github.com/repos/attacker/dmx-money-2/releases", "https://api.github.com@evil.example/repos/thefrcrazy/dmx-money-2/releases"] { precondition(!Self.isTrustedFeed(URL(string: value)!)) }
        precondition(!Self.isTrustedDownload(URL(string: "https://github.com/attacker/dmx-money-2/releases/download/v2.1.0/fixture.dmg")!))
        precondition(!Self.isTrustedDownload(URL(string: "https://github.com/thefrcrazy/dmx-money-2/releases/download/v2.1.0/fixture.dmg?redirect=evil")!))
        precondition(!AppInfo.isVersion("", newerThan: "2.0.0"))
        precondition(!AppInfo.isVersion("2.1.0+ignored", newerThan: "2.1.0"))
        print("PASS: release compatibility, SemVer, prerelease policy and trusted origins")
    }

    func testDownload(_ url: URL) {
        performDownloadAndRestart(build: Feed.Build(url: url, minimumSystemVersion: nil), version: "test")
    }
}
if CommandLine.arguments[1] == "policy" {
    UpdateChecker.shared.testReleasePolicy()
    exit(0)
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
test_installer_rollback()
test_entitlement_selection()
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
        for mode in ('policy', 'success', 'error', 'cancel'):
            subprocess.run([str(binary), mode, f'http://127.0.0.1:{server.server_port}/{mode}'], env=env, check=True, timeout=15)
finally:
    server.shutdown()
