import AppKit

// Cycle de vie AppKit sans storyboard : le menu et les fenêtres sont construits en code.
let application = NSApplication.shared
let delegate = AppDelegate()
application.delegate = delegate
_ = NSApplicationMain(CommandLine.argc, CommandLine.unsafeArgv)
