#!/usr/bin/env python3
"""Render actual journal sources with 30k synthetic rows, without FFI or user data.

The frozen SwiftUI baseline is decec654. Financial collaborators are deliberately
stubbed: this measures table rendering, not database, gesture FPS or whole-app UX.
"""
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = Path(__file__).parent / "fixtures/macos-journal"


def main():
    if platform.system() != "Darwin":
        raise SystemExit("This AppKit fixture requires macOS 26 or later.")
    output = os.environ.get("DMXMONEY_JOURNAL_TEST_OUTPUT")
    work = Path(output) if output else Path(tempfile.mkdtemp(prefix="dmx-journal-render-"))
    work.mkdir(parents=True, exist_ok=True)
    for source in FIXTURES.glob("*.swift"):
        shutil.copy2(source, work / source.name)
    controller = (ROOT / "apple/DmxMoney-macOS-Shared/JournalViewController.swift").read_text()
    start = controller.index("// MARK: - En-tête et état vide")
    end = controller.index("final class JournalTableView", start)
    # Only the legacy header/empty-view collaborators are stubbed; controller and
    # reusable cells, sorting, edit tokens and dismantling are production code.
    (work / "AfterController.swift").write_text(controller[:start] + controller[end:])
    main_source = (work / "main.swift").read_text()
    start = main_source.index("// BEGIN CONTROLLER ASSERTIONS")
    end = main_source.index("// END CONTROLLER ASSERTIONS", start)
    (work / "baseline").mkdir(exist_ok=True)
    (work / "baseline/main.swift").write_text(main_source[:start] + main_source[end:])
    flags = ["xcrun", "swiftc", "-O", "-swift-version", "5", "-target",
             f"{platform.machine()}-apple-macos26.0", "-module-cache-path", str(work / "cache")]

    def run(args, label):
        with (work / f"{label}.log").open("w") as log:
            subprocess.run(args, check=True, stdout=log, stderr=subprocess.STDOUT, timeout=180)

    run(flags + ["-emit-library", "-emit-module", "-module-name", "DmxKit",
                 str(work / "DmxKit.swift"), "-o", str(work / "libDmxKit.dylib"),
                 "-emit-module-path", str(work / "DmxKit.swiftmodule")], "module")
    links = ["-I", str(work), "-L", str(work), "-lDmxKit", "-Xlinker", "-rpath", "-Xlinker", str(work)]
    for mode in ("before", "after"):
        sources = ([work / "BeforeJournal.swift", work / "baseline/main.swift"] if mode == "before" else
                   [ROOT / "apple/DmxMoney-macOS-Modern/ModernJournal.swift",
                    work / "AfterController.swift", work / "main.swift"])
        run(flags + links + list(map(str, sources)) + [str(work / "Helpers.swift"), "-o", str(work / mode)], f"compile-{mode}")
    reports = {}
    for mode in ("before", "after"):
        with (work / f"{mode}.json").open("w") as result, (work / f"{mode}.stderr").open("w") as errors:
            subprocess.run([str(work / mode), str(work / mode)], check=True, stdout=result, stderr=errors, timeout=60)
        reports[mode] = json.loads((work / f"{mode}.json").read_text())
        assert reports[mode]["rows"] == 30000 and reports[mode]["tableDetached"]
    # Isolate edits/sort/selection from timing: the measured exit immediately follows
    # the same scroll sequence in both variants. The functional run is a fresh process.
    with (work / "functional.json").open("w") as result, (work / "functional.stderr").open("w") as errors:
        subprocess.run([str(work / "after"), str(work / "functional"), "--functional"],
                       check=True, stdout=result, stderr=errors, timeout=60)
    functional = json.loads((work / "functional.json").read_text())
    assert functional["functionalAssertions"] >= 11 and functional["tableDetached"]
    reports["functional"] = functional
    # No timing gate: CI host load and font/window caches vary.
    (work / "comparison.json").write_text(json.dumps(reports, indent=2))
    print(json.dumps(reports, indent=2))
    print(f"Proofs: {work}")
    # Keep small proof files and screenshots, remove compilation products.
    for name in ("cache", "before", "after", "libDmxKit.dylib", "DmxKit.swiftmodule", "DmxKit.swiftdoc", "DmxKit.swiftsourceinfo"):
        path = work / name
        if path.is_dir():
            shutil.rmtree(path)
        elif path.exists():
            path.unlink()


if __name__ == "__main__":
    main()
