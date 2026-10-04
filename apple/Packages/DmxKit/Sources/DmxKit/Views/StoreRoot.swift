import SwiftUI
import Darwin

/// Racine des vues hébergées : injecte le store et applique la couleur d'accentuation choisie.
public struct StoreRoot<Content: View>: View {
    @Environment(\.colorScheme) private var inheritedColorScheme
    @ObservedObject private var store: AppStore
    private let content: Content

    public init(store: AppStore, @ViewBuilder content: () -> Content) {
        self.store = store
        self.content = content()
    }

    public var body: some View {
        content
            .environmentObject(store)
            .environment(\.colorScheme, resolvedColorScheme)
            .accentColor(DmxColors.accent(store.settings.accentColor))
    }
    // AppKit changes its appearance separately. Explicitly pass forced themes to every
    // hosted SwiftUI page, including views retained while they are off screen on Catalina.
    private var resolvedColorScheme: ColorScheme {
        switch store.settings.theme {
        case .light: return .light
        case .dark: return .dark
        case .system: return inheritedColorScheme
        }
    }
}

/// Lecture des fichiers importés (relevés bancaires souvent en Windows-1252).
public enum FileText {
    public static let maximumBytes = 16 * 1024 * 1024
    public static let maximumBackupBytes = 64 * 1024 * 1024
    private static let queue = DispatchQueue(label: "com.dmxmoney.import-file", qos: .userInitiated)

    public enum ReadError: LocalizedError {
        case tooLarge(Int), unreadable
        public var errorDescription: String? {
            switch self {
            case let .tooLarge(limit): return "Le fichier dépasse la limite de \(limit / (1024 * 1024)) Mio."
            case .unreadable: return "Le fichier n'a pas pu être lu."
            }
        }
    }

    public static func read(_ url: URL) -> String? {
        try? readChecked(url)
    }

    public static func readChecked(_ url: URL) throws -> String {
        guard url.isFileURL, try url.resourceValues(forKeys: [.isRegularFileKey]).isRegularFile == true else { throw ReadError.unreadable }
        let limit = ["dmx", "json"].contains(url.pathExtension.lowercased()) ? maximumBackupBytes : maximumBytes
        let file = try FileHandle(forReadingFrom: url)
        defer { file.closeFile() }
        // A size attribute alone is insufficient: the file can grow between stat and read.
        var metadata = stat()
        guard fstat(file.fileDescriptor, &metadata) == 0, metadata.st_mode & S_IFMT == S_IFREG else { throw ReadError.unreadable }
        var data = Data()
        var buffer = [UInt8](repeating: 0, count: 64 * 1024)
        while data.count <= limit {
            let readCount = buffer.withUnsafeMutableBytes {
                Darwin.read(file.fileDescriptor, $0.baseAddress, min($0.count, limit + 1 - data.count))
            }
            if readCount == 0 { break }
            if readCount < 0 {
                if errno == EINTR { continue }
                throw ReadError.unreadable
            }
            data.append(contentsOf: buffer.prefix(readCount))
        }
        guard data.count <= limit else { throw ReadError.tooLarge(limit) }
        guard let text = decode(data) else { throw ReadError.unreadable }
        return text
    }

    public static func readAsync(_ url: URL, completion: @escaping (Result<String, Error>) -> Void) {
        queue.async {
            let access = url.startAccessingSecurityScopedResource()
            defer { if access { url.stopAccessingSecurityScopedResource() } }
            let result = Result { try readChecked(url) }
            DispatchQueue.main.async { completion(result) }
        }
    }

    public static func decode(_ data: Data) -> String? {
        for encoding in [String.Encoding.utf8, .windowsCP1252, .isoLatin1] {
            if let text = String(data: data, encoding: encoding) {
                return text.hasPrefix("\u{FEFF}") ? String(text.dropFirst()) : text
            }
        }
        return nil
    }
}
