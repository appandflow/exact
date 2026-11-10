import QuartzCore

/// An image's `placeholder` and `fadeDuration` props. The raster loader keeps
/// one source per view, so a view with a stand-in asks for it first and for
/// its own source once the stand-in shows; the source then replaces it with a
/// cross-fade. A source that lands within `quick` of being asked, as a cache
/// hit does, or one this process already showed, shows at once, over its
/// stand-in too.
final class ImageArrivals {
    static let shared = ImageArrivals()
    static let quick: CFTimeInterval = 0.05

    private struct Pending {
        let source: String
        let generation: Int
        var asked: CFTimeInterval
        var standIn: Bool
    }
    private var pending: [ObjectIdentifier: Pending] = [:]
    /// Sources this process has shown: asked again, they come from a cache, so
    /// their stand-in would only flash.
    private var shown = Set<String>()

    /// What `view` asks the loader for when its source becomes `source`.
    func asking(_ view: NodeView, for source: String) -> String {
        let standIn = view.props["placeholder"].flatMap { $0.isEmpty || $0 == source || $0.hasPrefix("symbol:") ? nil : $0 }
        let first = view.raster == nil && !shown.contains(source) ? standIn : nil
        pending[ObjectIdentifier(view)] = Pending(source: source, generation: view.loadGeneration, asked: CACurrentMediaTime(), standIn: first != nil)
        return first ?? source
    }

    /// `view` is about to show the raster its load asked for: a fade goes on
    /// before the change, whose transaction `applyImageLayer` commits.
    func arriving(_ view: NodeView) {
        let key = ObjectIdentifier(view)
        guard var item = pending[key], item.source == view.imageSource, item.generation == view.loadGeneration else { return }
        if item.standIn {
            item.standIn = false; pending[key] = item
            // After the loader's pass that delivered the stand-in: asking from
            // inside it would replace the interest that pass is still settling.
            DispatchQueue.main.async { [weak self, weak view] in
                guard let self, let view, let session = view.presenter?.session, view.imageSource == item.source, view.loadGeneration == item.generation else { return }
                // The source is asked now: in hand within `quick`, as from a cache, it replaces the stand-in at once.
                pending[key]?.asked = CACurrentMediaTime()
                session.rasters.load(view, source: item.source, resolver: session.app.resolver)
            }
            fade(view, crossing: CACurrentMediaTime() - item.asked > Self.quick)
            return
        }
        pending[key] = nil
        // One this process showed comes from a cache however long its decode takes: it shows at once.
        let again = !shown.insert(item.source).inserted
        fade(view, crossing: !again && CACurrentMediaTime() - item.asked > Self.quick)
    }

    func forget(_ view: NodeView) { pending[ObjectIdentifier(view)] = nil }

    private func fade(_ view: NodeView, crossing: Bool) {
        guard crossing, let ms = view.props["fadeDuration"].flatMap(Double.init), ms > 0 else { return }
        let t = CATransition()
        t.type = .fade
        t.duration = ms / 1000
        t.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
        // Its image layer cross-fades; before the first raster there is none,
        // and the view's own layer fades the image in.
        let target: CALayer? = view.imageLayer ?? view.layer
        target?.add(t, forKey: "exactImageFade")
    }
}
