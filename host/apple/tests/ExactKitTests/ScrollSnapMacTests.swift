#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// LLP 1008 §9 on AppKit: a trackpad's lift eases to the rest the shared
/// rule picks from where the momentum would carry the scroll, and a mouse
/// wheel's notches rest there once they stop.
final class ScrollSnapMacTests: XCTestCase {
    private var window: NSWindow?
    private var presenter: Presenter?
    override func tearDown() { window?.close(); window = nil; presenter = nil; super.tearDown() }

    /// A 300-wide scroller of `areas` (alignment, start, length) along x, or y
    /// when `vertical`, each `scroll-snap-stop: stop`.
    private func strip(_ areas: [(String, Double, Double)], type: String = "x mandatory", stop: String = "normal",
                       vertical: Bool = false) throws -> (NodeView, ChainingScrollView) {
        _ = NSApplication.shared
        let p = Presenter()
        presenter = p
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 300, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        self.window = window
        var ops: [[String: Any]] = [["op": "create", "id": 1, "kind": "view", "style": [
            "overflow_x": vertical ? "hidden" : "scroll", "overflow_y": vertical ? "scroll" : "hidden", "scroll_snap_type": type]]]
        for (i, (align, at, length)) in areas.enumerated() {
            ops.append(["op": "create", "id": 10 + i, "kind": "view", "style": ["scroll_snap_align": align, "scroll_snap_stop": stop]])
            ops.append(["op": "frame", "id": 10 + i, "x": vertical ? 0.0 : at, "y": vertical ? at : 0.0, "w": vertical ? 100.0 : length, "h": vertical ? length : 100.0])
        }
        ops += [["op": "children", "id": 1, "ids": areas.indices.map { 10 + $0 }], ["op": "roots", "ids": [1]],
                ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": vertical ? 100.0 : 300.0, "h": vertical ? 300.0 : 100.0],
                ["op": "content", "id": 1, "w": vertical ? 100.0 : 1200.0, "h": vertical ? 1200.0 : 100.0]]
        p.apply(wireBatch(ops))
        let node = try XCTUnwrap(p.views[1])
        return (node, try XCTUnwrap(node.scroll))
    }

    private func spin(until done: () -> Bool) {
        let end = Date().addingTimeInterval(3)
        while !done(), Date() < end { RunLoop.main.run(until: Date().addingTimeInterval(0.02)) }
    }

    func testTheRuleRestsAtTheNearestPositionOnEitherAxis() throws {
        for vertical in [false, true] {
            let (node, _) = try strip([("start", 0, 150), ("center", 300, 150), ("end", 600, 150), ("start", 750, 450)],
                                      type: vertical ? "y mandatory" : "x mandatory", vertical: vertical)
            for (projected, expected): (CGFloat, CGFloat) in [(40, 0), (200, 225), (500, 450), (820, 820), (950, 900)] {
                let at = vertical ? CGPoint(x: 0, y: projected) : CGPoint(x: projected, y: 0)
                let rest = try XCTUnwrap(node.snappedOrigin(at, release: .zero))
                XCTAssertEqual(vertical ? rest.y : rest.x, expected, accuracy: 0.01, "\(vertical) \(projected)")
            }
        }
    }

    func testAlwaysAndProximity() throws {
        let cards = (0..<6).map { ("start", Double($0) * 150, 150.0) }
        let (always, _) = try strip(cards, stop: "always")
        XCTAssertEqual(always.snappedOrigin(CGPoint(x: 620, y: 0), release: CGPoint(x: 20, y: 0))?.x, 150, "not past the next `always` area")
        let (normal, _) = try strip(cards)
        XCTAssertEqual(normal.snappedOrigin(CGPoint(x: 620, y: 0), release: CGPoint(x: 20, y: 0))?.x, 600)
        let (near, _) = try strip([("start", 0, 150), ("start", 400, 150), ("start", 800, 150)], type: "x")
        XCTAssertEqual(near.snappedOrigin(CGPoint(x: 330, y: 0), release: .zero)?.x, 400, "within a third of the snapport")
        XCTAssertNil(near.snappedOrigin(CGPoint(x: 200, y: 0), release: .zero), "`proximity` leaves a rest far from every area")
    }

    func testALiftEasesToTheRestItsMomentumProjects() throws {
        let (node, sv) = try strip((0..<8).map { ("start", Double($0) * 150, 150.0) })
        sv.contentView.scroll(to: NSPoint(x: 40, y: 0))
        sv.reflectScrolledClipView(sv.contentView)
        // 0.5 pt/ms projects about 250 points on: 290, nearest 300.
        XCTAssertTrue(node.snapAtLift(velocity: CGPoint(x: 0.5, y: 0)))
        XCTAssertTrue(sv.snapSettling, "the momentum that follows is dropped")
        spin { sv.contentView.bounds.minX == 300 && !sv.snapSettling }
        XCTAssertEqual(sv.contentView.bounds.minX, 300, accuracy: 0.01)
        XCTAssertFalse(sv.snapSettling)
    }

    /// CSS Scroll Snap 1 §5.5: a batch that moves the areas keeps the
    /// scroller on the area it rested on.
    func testALayoutChangeKeepsTheAreaItRestedOn() throws {
        let (node, sv) = try strip((0..<8).map { ("start", Double($0) * 150, 150.0) })
        sv.contentView.scroll(to: try XCTUnwrap(node.snappedOrigin(NSPoint(x: 280, y: 0), release: .zero)))
        XCTAssertEqual(sv.contentView.bounds.minX, 300, accuracy: 0.01)
        node.resnap()
        XCTAssertEqual(sv.contentView.bounds.minX, 300, accuracy: 0.01, "nothing moved")
        presenter?.apply(wireBatch((0..<8).map { ["op": "frame", "id": 10 + $0, "x": Double($0) * 200, "y": 0.0, "w": 200.0, "h": 100.0] }
            + [["op": "content", "id": 1, "w": 1600.0, "h": 100.0]]))
        XCTAssertEqual(sv.contentView.bounds.minX, 400, accuracy: 0.01, "the third card, where it now is")
    }

    func testWheelNotchesRestOnceTheyStop() throws {
        let (node, sv) = try strip((0..<8).map { ("start", Double($0) * 150, 150.0) })
        sv.contentView.scroll(to: NSPoint(x: 100, y: 0))
        node.snapAfterWheel(from: .zero, now: false)
        XCTAssertEqual(sv.contentView.bounds.minX, 100, "not while the notches come")
        spin { sv.contentView.bounds.minX == 150 }
        XCTAssertEqual(sv.contentView.bounds.minX, 150, accuracy: 0.01)
    }
}
#endif
