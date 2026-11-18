#if os(iOS) || os(tvOS)
import UIKit

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

    #if os(iOS)
    /// What decides whether a drag on the top sheet scrolls its content or
    /// moves the sheet: the detent, the scroll views UIKit's sheet can
    /// track, and each scroll view in the sheet with its gesture and extent.
    static func scrolls(in controller: UIViewController) -> String {
        guard controller.isViewLoaded, let view = controller.view else { return "scrolls: no view" }
        var parts: [String] = []
        if let sheet = controller.sheetPresentationController {
            parts.append("detent=\(sheet.selectedDetentIdentifier?.rawValue ?? "nil") detents=\(sheet.detents.count) expands=\(sheet.prefersScrollingExpandsWhenScrolledToEdge)")
        }
        parts.append("frame=\(short(view.frame)) window=\(view.window != nil) subviews=\(view.subviews.map(name).joined(separator: ","))")
        parts.append("csv.bottom=\(id(controller.contentScrollView(for: .bottom))) csv.top=\(id(controller.contentScrollView(for: .top)))")
        for child in descendants(of: controller) {
            let top = (child as? UINavigationController)?.topViewController
            parts.append("child \(name(child)) csv.bottom=\(id(child.contentScrollView(for: .bottom)))" + (top.map { " top=\(name($0)) top.csv.bottom=\(id($0.contentScrollView(for: .bottom)))" } ?? ""))
        }
        parts += scrollViews(in: view).map(describe)
        return parts.joined(separator: " ; ")
    }

    static func describe(_ scroll: UIScrollView) -> String {
        let pan = scroll.panGestureRecognizer
        let frame = scroll.window.map { scroll.convert(scroll.bounds, to: $0) } ?? .null
        let chain = Array(sequence(first: scroll as UIView, next: \.superview))
        let hidden = chain.contains { $0.isHidden || $0.alpha < 0.01 }
        let interactive = chain.allSatisfy(\.isUserInteractionEnabled)
        return "sv \(id(scroll)) in=\(short(frame)) enabled=\(scroll.isScrollEnabled) pan=\(pan.isEnabled)/\(pan.state.rawValue) interactive=\(interactive) hidden=\(hidden) size=\(Int(scroll.contentSize.height)) bounds=\(Int(scroll.bounds.height)) offset=\(Int(scroll.contentOffset.y)) inset=\(Int(scroll.adjustedContentInset.top)),\(Int(scroll.adjustedContentInset.bottom)) tracking=\(scroll.isTracking) decel=\(scroll.isDecelerating)"
    }

    private static func descendants(of controller: UIViewController) -> [UIViewController] {
        controller.children.flatMap { [$0] + descendants(of: $0) }
    }

    private static func scrollViews(in view: UIView) -> [UIScrollView] {
        view.subviews.flatMap { sub in ((sub as? UIScrollView).map { [$0] } ?? []) + scrollViews(in: sub) }
    }

    static func id(_ object: AnyObject?) -> String {
        object.map { "\(name($0))@\(String(UInt(bitPattern: ObjectIdentifier($0).hashValue), radix: 16))" } ?? "nil"
    }
    static func name(_ object: AnyObject) -> String { String(describing: type(of: object)) }
    static func short(_ r: CGRect) -> String { r.isNull ? "null" : "\(Int(r.minX)),\(Int(r.minY)) \(Int(r.width))x\(Int(r.height))" }

    /// Every touch on the window while a sheet is up: the views it lands in,
    /// each recognizer that hears it and their states as it ends, so a drag
    /// the sheet takes instead of its content names the recognizer that won.
    /// It never recognizes, and cancels or delays no touch.
    final class TouchProbe: UIGestureRecognizer, UIGestureRecognizerDelegate {
        private let top: () -> UIViewController?
        private var heard: [UIGestureRecognizer] = []
        private var scroll: UIScrollView?
        private var start: CGPoint = .zero
        init(top: @escaping () -> UIViewController?) {
            self.top = top
            super.init(target: nil, action: nil)
            cancelsTouchesInView = false
            delaysTouchesBegan = false
            delaysTouchesEnded = false
            delegate = self
        }
        func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer) -> Bool { true }

        override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent) {
            guard numberOfTouches <= 1, let touch = touches.first, top() != nil, let window = view else { return }
            start = touch.location(in: window)
            heard = (touch.gestureRecognizers ?? []).filter { $0 !== self }
            let chain = Array(sequence(first: touch.view, next: { $0?.superview }).compactMap { $0 })
            scroll = chain.lazy.compactMap { $0 as? UIScrollView }.first
            SheetTrace.note("touch began at \(Int(start.x)),\(Int(start.y)) app=\(UIApplication.shared.applicationState.rawValue) view=\(chain.prefix(8).map(SheetTrace.name).joined(separator: "<")) heard=\(recognizers()) \(scroll.map(SheetTrace.describe) ?? "no scroll")")
        }
        override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent) { finish("ended", touches) }
        override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent) { finish("cancelled", touches) }
        private func finish(_ how: String, _ touches: Set<UITouch>) {
            if !heard.isEmpty, let window = view, let touch = touches.first {
                let end = touch.location(in: window)
                SheetTrace.note("touch \(how) moved \(Int(end.x - start.x)),\(Int(end.y - start.y)) app=\(UIApplication.shared.applicationState.rawValue) heard=\(recognizers()) \(scroll.map(SheetTrace.describe) ?? "no scroll") \(top().map(SheetTrace.scrolls) ?? "")")
            }
            heard = []
            scroll = nil
            state = .failed
        }
        private func recognizers() -> String {
            heard.map { "\(SheetTrace.name($0))[\(SheetTrace.name($0.view ?? $0))]\($0.isEnabled ? "" : "-off")/\($0.state.rawValue)" }.joined(separator: ",")
        }
    }
    #endif
}
#endif
