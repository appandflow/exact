// CSS scroll snap on UIKit (LLP 1008 §9, "Scroll snap"): UIKit projects
// where a drag comes to rest and owns the deceleration there; the snap
// position the rule picks (`ScrollSnap.swift`) replaces the projection.
#if os(iOS) || os(tvOS)
import UIKit

extension NodeView {
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
        targetContentOffset.pointee = snappedOffset(targetContentOffset.pointee, in: scrollView, release: scrollView.contentOffset)
    }

    /// UIKit's deceleration for a scroller: its fast rate where a snap is
    /// mandatory, as a paging scroller's, else its normal one.
    var snapDeceleration: UIScrollView.DecelerationRate {
        ScrollSnap.Kind(style["scroll_snap_type"]?.string)?.mandatory == true ? .fast : .normal
    }

    /// Where a scroll projected to rest at `proposed` rests, on each axis
    /// `scroll-snap-type` names: optionally only at the snap areas holding a
    /// target the agent reveals, and, released at `release`, not past an
    /// `always` area's position.
    func snappedOffset(_ proposed: CGPoint, in scrollView: UIScrollView, revealing target: UIView? = nil, release: CGPoint? = nil) -> CGPoint {
        snapTarget(proposed, in: scrollView, revealing: target, release: release) ?? proposed
    }

    /// `snappedOffset`'s, or `nil` when no axis snaps: no snap type, no snap
    /// area, or a `proximity` snap too far from every area.
    func snapTarget(_ proposed: CGPoint, in scrollView: UIScrollView, revealing target: UIView? = nil, release: CGPoint? = nil, keeping: Bool = false) -> CGPoint? {
        guard let kind = ScrollSnap.Kind(style["scroll_snap_type"]?.string) else { return nil }
        var result = proposed, snapped = false
        if kind.x, let x = rest(false, kind, proposed.x, in: scrollView, revealing: target, release: release?.x ?? proposed.x, keeping: keeping) { result.x = x; snapped = true }
        if kind.y, let y = rest(true, kind, proposed.y, in: scrollView, revealing: target, release: release?.y ?? proposed.y, keeping: keeping) { result.y = y; snapped = true }
        return snapped ? result : nil
    }

    /// CSS Scroll Snap 1 §5.5: a mandatory scroller at rest on a snap area
    /// follows it when a batch moves it, and when it went, rests on the
    /// position nearest (`ScrollSnap.resnap`).
    func resnap() {
        guard let sv = scrollView, !sv.isTracking, !sv.isDecelerating, ScrollSnap.Kind(style["scroll_snap_type"]?.string)?.mandatory == true,
              let to = snapTarget(sv.contentOffset, in: sv, keeping: true).map({ PixelGrid.offset($0, in: sv) }),
              abs(to.x - sv.contentOffset.x) > 0.5 || abs(to.y - sv.contentOffset.y) > 0.5 else { return }
        sv.contentOffset = to
    }

    private func rest(_ vertical: Bool, _ kind: ScrollSnap.Kind, _ proposed: CGFloat, in scrollView: UIScrollView, revealing target: UIView?, release: CGFloat, keeping: Bool) -> CGFloat? {
        // The snapport is the padding box less UIKit's insets and
        // `scroll-padding` (§4.1); positions are where its start is in the
        // content, which starts at the border box: an offset plus `shift`.
        let size = vertical ? scrollView.bounds.height : scrollView.bounds.width
        let inset = scrollView.adjustedContentInset
        let padding = { (side: String) in self.style["scroll_padding_\(side)"]?.length(basis: size) ?? 0 }
        let border = { (side: String) in self.number("border_width_\(side)", self.number("border_width")) }
        let (first, last) = vertical ? ("top", "bottom") : ("left", "right")
        let lead = (vertical ? inset.top : inset.left), trail = (vertical ? inset.bottom : inset.right)
        let shift = lead + border(first) + padding(first)
        let port = size - lead - trail - border(first) - border(last) - padding(first) - padding(last)
        guard port > 0 else { return nil }
        let content = vertical ? scrollView.contentSize.height : scrollView.contentSize.width
        let range = (-lead + shift)...max(-lead, content + trail - size) + shift
        var areas: [ScrollSnap.Area] = [], held: [ScrollSnap.Area] = []
        func visit(_ view: UIView) {
            for case let node as NodeView in view.subviews where !node.isHidden {
                if let align = ScrollSnap.align(node.style["scroll_snap_align"]?.string, vertical: vertical) {
                    // The area is the border box grown by `scroll-margin` (§4.3).
                    let rect = IntoView.inset(node.convert(node.bounds, to: scrollView), style: node.style, family: "scroll_margin", outward: true)
                    let area = ScrollSnap.area(lo: vertical ? rect.minY : rect.minX, hi: vertical ? rect.maxY : rect.maxX, align: align,
                                               stop: node.style["scroll_snap_stop"]?.string == "always", port: port, range: range, id: node.id)
                    areas.append(area)
                    if let target, target === node || target.isDescendant(of: node) { held.append(area) }
                }
                // A nested scroll container captures its own snap areas, `hidden`
                // included without a UIScrollView; `clip` does not (CSS Overflow 3 §3.1).
                if node.scrollView == nil && !ScrollSnap.ownsAreas(node.style["overflow_x"]?.string, node.style["overflow_y"]?.string) { visit(node.container) }
            }
        }
        visit(scrollView)
        let wanted = proposed + shift
        // A virtualized list's rows past its mounted ones (LLP 1008 §9, as amended for #219).
        let distance = areas.map { abs(min($0.to, max($0.from, wanted)) - wanted) }.min() ?? .infinity
        if distance > 0, held.isEmpty, let entry = presenter?.collections.entries[id], entry.snapshot.horizontal != vertical,
           let position = presenter?.session?.runtime.collectionSnap(id, proposed: Double(wanted), port: Double(port),
                minimum: Double(range.lowerBound), maximum: Double(range.upperBound), distance: Double(distance)) {
            areas.append(ScrollSnap.Area(from: CGFloat(position), to: CGFloat(position), stop: false))
        }
        let axis = vertical ? 1 : 0, sv = scrollView as? ScrollView
        let rest = keeping
            ? ScrollSnap.resnap(areas, rested: sv?.snapRested[axis] ?? .init(), current: wanted, mandatory: kind.mandatory, port: port)
            : ScrollSnap.rest(held.isEmpty ? areas : held, projected: wanted, release: release + shift, mandatory: kind.mandatory || !held.isEmpty, port: port)
        guard let rest else { return nil }
        sv?.snapRested[axis] = ScrollSnap.Rested(rest, in: areas)
        return rest - shift
    }
}
#endif
