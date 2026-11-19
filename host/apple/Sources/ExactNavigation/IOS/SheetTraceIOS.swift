#if os(iOS) || os(tvOS)
import UIKit
import ExactKit

/// Each sheet dismissal decision and what it read, for a refused swipe seen
/// on a phone: lines in the unified log (`exact-sheet`) and in
/// `app:/cache/exact-sheet-trace.log`, which an app shares with the `fs.read`
/// grant. The file keeps its last 200 to 400 KB.
enum SheetTrace {
    static let path = "app:/cache/exact-sheet-trace.log"
    private static let limit = 400_000
    private static var opened = false
    private static let stamp: ISO8601DateFormatter = {
        let f = ISO8601DateFormatter()
        f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return f
    }()

    static func note(_ line: String) {
        NSLog("exact-sheet %@", line)
        guard let url = AppFiles.url(path) else { return }
        var text = ""
        if !opened {
            opened = true
            let info = Bundle.main.infoDictionary
            text += "\(stamp.string(from: Date())) launch \(UIDevice.current.systemName) \(UIDevice.current.systemVersion) build \(info?["CFBundleVersion"] as? String ?? "?")\n"
        }
        text += "\(stamp.string(from: Date())) \(line)\n"
        let data = Data(text.utf8)
        guard let handle = try? FileHandle(forWritingTo: url) else {
            try? FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            try? data.write(to: url)
            return
        }
        defer { try? handle.close() }
        if (try? handle.seekToEnd()) ?? 0 > limit, let all = try? Data(contentsOf: url) {
            try? (all.suffix(limit / 2) + data).write(to: url)
            return
        }
        try? handle.write(contentsOf: data)
    }

    /// The presentations' state at each edge of the app's lifecycle, and a turn after it.
    static func watch(_ host: ModalHost) -> [NSObjectProtocol] {
        let edges: [(Notification.Name, String)] = [
            (UIApplication.willResignActiveNotification, "willResignActive"),
            (UIApplication.didEnterBackgroundNotification, "didEnterBackground"),
            (UIApplication.willEnterForegroundNotification, "willEnterForeground"),
            (UIApplication.didBecomeActiveNotification, "didBecomeActive"),
            (UIApplication.didReceiveMemoryWarningNotification, "memoryWarning"),
        ]
        return edges.map { name, edge in
            NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main) { [weak host] _ in
                host?.traceLayers(edge)
                DispatchQueue.main.async { [weak host] in host?.traceLayers(edge + " +1 turn") }
            }
        }
    }
}
#endif
