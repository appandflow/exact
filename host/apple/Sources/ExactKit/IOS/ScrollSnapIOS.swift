// CSS scroll snap on UIKit (LLP 1008, "Horizontal scroll snap"): UIKit
// projects where a drag comes to rest; the nearest captured snap position
// replaces it.
#if os(iOS) || os(tvOS)
import UIKit

extension NodeView {
    /// CSS's admitted `x mandatory` or `y mandatory` / `start`, `center` and `end` scroll snap. UIKit supplies
    /// the projected resting offset and owns the resulting deceleration.
    package func scrollViewWillEndDragging(_ scrollView: UIScrollView, withVelocity velocity: CGPoint, targetContentOffset: UnsafeMutablePointer<CGPoint>) {
        guard scrollView === self.scrollView else { return }
        // `scrolldragend` (Charlie, 2026-11-10): UIKit's velocity, points per
        // millisecond and positive as the offset grows, as the web's
        // scrollLeft/scrollTop, and the rest decided below, in those coordinates.
        defer {
            if handlers.contains("scrolldragend"), velocity.x.isFinite, velocity.y.isFinite {
                let rest = targetContentOffset.pointee
                presenter?.scrollDrag(id, (velocity, CGPoint(x: rest.x, y: rest.y + scrollTopInset(scrollView))))
            }
        }
        #if os(iOS)
        // A painted swipe row's full swipe decides where it rests (PaintedSwipesIOS.swift).
        defer { if swipeOwner { presenter?.swipeActions.painted.willEnd(self, scrollView, target: targetContentOffset) } }
        #endif
        targetContentOffset.pointee = snappedOffset(targetContentOffset.pointee, in: scrollView, from: scrollView.contentOffset, velocity: velocity)
    }

    /// The native resting position, optionally constrained to the snap area
    /// holding a target the agent reveals: along x for `x mandatory`, along y
    /// for `y mandatory`. A drag released at `origin` does not pass over the
    /// position of an area that says `scroll-snap-stop: always` (CSS Snap §6.2).
    /// A flick (UIKit's `velocity`, points per millisecond) reaches at least the
    /// next position its way, however short its projected travel.
    func snappedOffset(_ proposed: CGPoint, in scrollView: UIScrollView, revealing target: UIView? = nil, from origin: CGPoint? = nil, velocity: CGPoint = .zero) -> CGPoint {
        let type = style["scroll_snap_type"]?.string
        guard type == "x mandatory" || type == "y mandatory" else { return proposed }
        let vertical = type == "y mandatory"
        let port = vertical ? scrollView.bounds.height : scrollView.bounds.width
        let maximum = max(0, (vertical ? scrollView.contentSize.height : scrollView.contentSize.width) - port)
        let wanted = vertical ? proposed.y : proposed.x
        var positions: [CGFloat] = [], targetPositions: [CGFloat] = [], stops: [CGFloat] = []
        func visit(_ view: UIView) {
            for case let node as NodeView in view.subviews where !node.isHidden {
                let align = node.style["scroll_snap_align"]?.string
                if align == "start" || align == "end" || align == "center" {
                    let rect = node.convert(node.bounds, to: scrollView)
                    let (lo, mid, hi, size) = vertical ? (rect.minY, rect.midY, rect.maxY, rect.height) : (rect.minX, rect.midX, rect.maxX, rect.width)
                    // A snap area larger than the viewport can be explored
                    // freely while it covers the viewport (CSS Snap §5.2.2);
                    // a smaller one rests with its start, centre or end at
                    // the viewport's.
                    let wide = size >= port
                    let at = wide ? lo : align == "end" ? hi - port : align == "center" ? mid - port / 2 : lo
                    let start = min(maximum, max(0, at))
                    let end = wide ? min(maximum, max(start, hi - port)) : start
                    let position = min(end, max(start, wanted))
                    positions.append(position)
                    if node.style["scroll_snap_stop"]?.string == "always" { stops.append(position) }
                    if let target, target === node || target.isDescendant(of: node) { targetPositions.append(position) }
                }
                // A nested scroll container captures its own snap areas.
                if node.scrollView == nil && !node.scrollDormant && (node.style["scroll_snap_type"]?.string ?? "none") == "none" { visit(node.container) }
            }
        }
        visit(scrollView)
        let choices = targetPositions.isEmpty ? positions : targetPositions
        var result = proposed
        if var nearest = choices.min(by: { abs($0 - wanted) < abs($1 - wanted) }) {
            if let origin {
                let from = vertical ? origin.y : origin.x
                let speed = vertical ? velocity.y : velocity.x
                if speed > 0.3, nearest <= from + 0.5, let next = choices.filter({ $0 > from + 0.5 }).min() { nearest = next }
                if speed < -0.3, nearest >= from - 0.5, let next = choices.filter({ $0 < from - 0.5 }).max() { nearest = next }
                if nearest > from + 0.5, let stop = stops.filter({ $0 > from + 0.5 }).min() { nearest = min(nearest, stop) }
                if nearest < from - 0.5, let stop = stops.filter({ $0 < from - 0.5 }).max() { nearest = max(nearest, stop) }
            }
            if vertical { result.y = nearest } else { result.x = nearest }
        }
        return result
    }
}
#endif
