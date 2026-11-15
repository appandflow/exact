// A background of repeating linear gradients as tiles on sublayers
// (`GradientTile.swift`), instead of `draw(_:)`'s bitmap of the whole box:
// a new size moves the tiles and draws nothing, where the bitmap was drawn
// again at every size (a sheet resized under a finger, every frame).
#if os(iOS) || os(tvOS)
import UIKit

/// A node's repeating gradients and the layers that tile them.
final class GradientTiles {
    let source: BatchValue
    let dark: Bool
    /// The view's tint, which a gradient's `AccentColor` stop takes.
    let tint: UIColor?
    /// Each gradient's period, first on top; nil when one is not a repeating
    /// linear gradient a tile can paint.
    let periods: [Gradient.Period]?
    /// Under the box's border and children, clipped to its outline: one
    /// layer of tiles for each gradient, the last first.
    var layer: CALayer?
    /// Each gradient's tile pictures by key (`Gradient.Period.key`), at a
    /// scale, tile size and start corner phase.
    var pictures: [(scale: CGFloat, width: Int, height: Int, phase: CGFloat, images: [Int: CGImage])] = []

    init(source: BatchValue, dark: Bool, tint: UIColor?, periods: [Gradient.Period]?) {
        self.source = source; self.dark = dark; self.tint = tint; self.periods = periods
    }
}

extension NodeView {
    /// The background's gradients, when every one is a repeating linear
    /// gradient whose period is the same at every size.
    private var gradientPeriods: [Gradient.Period]? {
        guard let source = style["background_image"] else { return nil }
        let dark = drawsDark, tint = source.namesTint ? gradientTint : nil
        if let tiles = gradientTiles, tiles.source == source, tiles.dark == dark, tiles.tint == tint { return tiles.periods }
        let layers = Gradient.layers(source, tint: tint), periods = layers.compactMap { $0.period(dark: dark) }
        let made = GradientTiles(source: source, dark: dark, tint: tint, periods: !layers.isEmpty && periods.count == layers.count ? periods : nil)
        made.layer = gradientTiles?.layer
        gradientTiles = made
        return made.periods
    }

    /// Whether tiles paint the background's gradients: repeating ones in
    /// the border box, scrolling with it.
    var gradientTiled: Bool {
        !gradientFixed && backgroundClip == "border-box" && surface == nil && gradientPeriods != nil
    }

    /// The tiles under the box's border and children, over its fill, for a
    /// box nothing else draws (`applyBoxLayer`). A new size moves them and
    /// draws only a picture no tile had (`Gradient.Period.layout`).
    func applyGradientTiles() {
        let scale = contentScaleFactor
        guard !cssVisibilityHidden, !drawsPaint, gradientTiled, let tiles = gradientTiles, let periods = tiles.periods,
              bounds.width > 0, bounds.height > 0 else {
            gradientTiles?.layer?.removeFromSuperlayer()
            gradientTiles?.layer = nil
            gradientTiles?.pictures = []
            return
        }
        CATransaction.begin(); CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        let container = tiles.layer ?? CALayer()
        tiles.layer = container
        if container.superlayer !== layer { insertBoxSublayer(container) }
        if container.frame != layer.bounds { container.frame = layer.bounds }
        if layer.cornerRadius == 0 && cornerRadii(in: bounds).contains(where: { $0 > 0 }) {
            let outline = (container.mask as? CAShapeLayer) ?? CAShapeLayer()
            let path = roundedPath(in: bounds).cgPath
            if outline.path != path { outline.path = path }
            if container.mask !== outline { container.mask = outline }
            if container.cornerRadius != 0 { container.cornerRadius = 0 }
        } else {
            if container.mask != nil { container.mask = nil }
            if container.cornerRadius != layer.cornerRadius { container.cornerRadius = layer.cornerRadius }
            if container.maskedCorners != layer.maskedCorners { container.maskedCorners = layer.maskedCorners }
            if container.cornerCurve != layer.cornerCurve { container.cornerCurve = layer.cornerCurve }
        }
        if !container.masksToBounds { container.masksToBounds = true }
        var holders = container.sublayers ?? []
        while holders.count < periods.count { let h = CALayer(); container.addSublayer(h); holders.append(h) }
        while holders.count > periods.count { holders.removeLast().removeFromSuperlayer() }
        if tiles.pictures.count != periods.count { tiles.pictures = Array(repeating: (0, 0, 0, 0, [:]), count: periods.count) }
        let box = gradientBox
        for (i, period) in periods.reversed().enumerated() {
            guard let at = period.layout(box: box, bounds: bounds, scale: scale) else { continue }
            var cache = tiles.pictures[i]
            if cache.scale != scale || cache.width != at.width || cache.height != at.height || abs(cache.phase - at.phase) * scale >= 1.0 / 256 {
                cache = (scale, at.width, at.height, at.phase, [:])
            }
            let (tw, th) = (CGFloat(at.width) / scale, CGFloat(at.height) / scale)
            let holder = holders[i]
            let frame = CGRect(x: at.origin.x, y: at.origin.y, width: CGFloat(at.columns) * tw, height: CGFloat(at.rows) * th)
            if holder.frame != frame { holder.frame = frame }
            var cells = holder.sublayers ?? []
            while cells.count < at.columns * at.rows { let c = CALayer(); holder.addSublayer(c); cells.append(c) }
            while cells.count > at.columns * at.rows { cells.removeLast().removeFromSuperlayer() }
            var used: Set<Int> = []
            for (k, cell) in cells.enumerated() {
                let (column, row) = (k % at.columns, k / at.columns)
                let key = period.key(at, column: column, row: row, scale: scale)
                used.insert(key)
                guard let image = cache.images[key] ?? period.image(at, key: key, scale: scale) else { continue }
                cache.images[key] = image
                let rect = CGRect(x: CGFloat(column) * tw, y: CGFloat(row) * th, width: tw, height: th)
                if cell.frame != rect { cell.frame = rect }
                if cell.contentsScale != scale { cell.contentsScale = scale }
                if (cell.contents as AnyObject?) !== image { cell.contents = image }
            }
            // Pictures a resize took out of view stay for one coming back, up to a bound.
            if cache.images.count > 2 * used.count + 16 { cache.images = cache.images.filter { used.contains($0.key) } }
            tiles.pictures[i] = cache
        }
    }
}
#endif
