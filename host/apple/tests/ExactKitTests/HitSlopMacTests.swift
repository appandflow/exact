#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// `-exact-hit-slop` (Charlie, 2026-11-10) on AppKit, whose own walk refuses
/// a point outside a view's frame: a small button takes a click past its
/// box, its frame unmoved; a sibling painted over the slop keeps its own
/// clicks; an `overflow` ancestor, or the button's own, clips it.
final class HitSlopMacTests: XCTestCase {
    private let slop: [String: Any] = ["hit_slop_top": 10, "hit_slop_right": 10, "hit_slop_bottom": 10, "hit_slop_left": 10]

    /// A screen (1) holding a box (4) that holds a 24-point button (2) and,
    /// after it, a sibling (3) whose box starts 6 points past its right edge.
    private func presenter(_ button: [String: Any], clip: Bool = false) -> (Presenter, (CGFloat, CGFloat) -> NSView?) {
        _ = NSApplication.shared
        let p = Presenter()
        p.viewport.frame = NSRect(x: 0, y: 0, width: 400, height: 400)
        let holder: [String: Any] = clip ? ["overflow_x": "hidden", "overflow_y": "hidden"] : [:]
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 4, "kind": "view", "style": holder],
            ["op": "create", "id": 2, "kind": "view", "handlers": ["press"], "style": button],
            ["op": "create", "id": 3, "kind": "view", "handlers": ["press"]],
            ["op": "children", "id": 4, "ids": [2, 3]], ["op": "children", "id": 1, "ids": [4]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "frame", "id": 4, "x": 100.0, "y": 100.0, "w": 100.0, "h": 24.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 24.0, "h": 24.0],
            ["op": "frame", "id": 3, "x": 30.0, "y": 0.0, "w": 40.0, "h": 24.0],
        ]))
        let root = p.views[1]!
        return (p, { x, y in root.hitTest(root.superview!.convert(NSPoint(x: x, y: y), from: root)) })
    }

    func testASlopTakesClicksPastTheBoxAndMovesNothing() throws {
        let (p, at) = presenter(slop)
        let button = try XCTUnwrap(p.views[2])
        XCTAssertEqual(button.frame, NSRect(x: 0, y: 0, width: 24, height: 24), "layout never sees the slop")
        for (x, y) in [(95.0, 112.0), (112.0, 92.0), (112.0, 132.0), (127.0, 112.0)] as [(CGFloat, CGFloat)] {
            XCTAssertTrue(at(x, y) === button, "in the slop: \(x), \(y)")
        }
        XCTAssertTrue(at(112, 112) === button, "its box")
        XCTAssertFalse(at(88, 112) === button, "past the slop")
        XCTAssertTrue(at(132, 112) === p.views[3], "the sibling paints over the slop where their boxes meet")
        let (plain, plainAt) = presenter([:])
        XCTAssertFalse(plainAt(95, 112) === plain.views[2], "no slop, no click")
    }

    func testAClipCutsTheSlop() throws {
        let (clipped, at) = presenter(slop, clip: true)
        XCTAssertFalse(at(95, 112) === clipped.views[2], "outside the clipping parent")
        XCTAssertTrue(at(127, 112) === clipped.views[2], "inside it")
        var own = slop
        own["overflow_x"] = "hidden"; own["overflow_y"] = "hidden"
        let (p, ownAt) = presenter(own)
        XCTAssertFalse(ownAt(95, 112) === p.views[2], "its own overflow clips its slop")
    }
}
#endif
