// `-exact-hit-slop` (Charlie, 2026-11-10; issues/20261021-scroll-drag-events-and-hit-slop.md):
// a node takes a pointer past its box, as a UIKit `point(inside:with:)`
// override or React Native's `hitSlop` does. Its children are asked first
// and the front sibling before it, so where boxes overlap the front one wins,
// slop and all; a clipping ancestor stops the walk before it is asked.
// Layout, paint and accessibility keep `bounds`.
#if os(macOS)
import AppKit
#else
import UIKit
#endif

extension NodeView {
    /// `bounds` grown by the hit slop: where a point reaches this node
    /// itself. A box that clips its overflow clips its own slop, as the
    /// browser clips the pseudo-element the web draws it with.
    var hitBounds: CGRect {
        let sides = ["top", "right", "bottom", "left"].map { max(0, style["hit_slop_\($0)"]?.length(basis: 0) ?? 0) }
        guard sides.contains(where: { $0 > 0 }),
              [style["overflow_x"], style["overflow_y"]].allSatisfy({ ($0?.string ?? "visible") == "visible" }) else { return bounds }
        return CGRect(x: bounds.minX - sides[3], y: bounds.minY - sides[0],
                      width: bounds.width + sides[1] + sides[3], height: bounds.height + sides[0] + sides[2])
    }

    #if os(macOS)
    /// This node at `point` (its superview's space) when only its slop holds
    /// it: AppKit's own walk refuses a point outside the frame. It is flipped,
    /// so the top slop is toward a smaller `y`.
    func slopHit(_ point: NSPoint) -> NSView? {
        let local = convert(point, from: superview)
        return !bounds.contains(local) && hitBounds.contains(local) ? self : nil
    }
    #endif
}
