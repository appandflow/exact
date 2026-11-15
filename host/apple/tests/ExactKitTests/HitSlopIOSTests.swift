#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// `-exact-hit-slop` (Charlie, 2026-11-10): a small button takes a touch
/// past its box, its frame unmoved; a sibling painted over the slop keeps
/// its own touches; an `overflow` ancestor, or the button's own, clips it.
///   bun host/apple/build.mjs --test --ios
final class HitSlopIOSTests: XCTestCase {
    private var window: UIWindow!

    private func presenter(_ button: [String: Any], clip: Bool = false) -> Presenter {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        // A screen (1) holding a box (4) that holds a 24-point button (2) and,
        // after it, a sibling (3) whose box starts 6 points past its right edge.
        let holder: [String: Any] = clip ? ["overflow_x": "hidden", "overflow_y": "hidden"] : [:]
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 4, "kind": "view", "style": holder],
            ["op": "create", "id": 2, "kind": "view", "handlers": ["press"], "props": ["testId": "small"], "style": button],
            ["op": "create", "id": 3, "kind": "view", "handlers": ["press"], "props": ["testId": "next"]],
            ["op": "children", "id": 4, "ids": [2, 3]], ["op": "children", "id": 1, "ids": [4]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "frame", "id": 4, "x": 100.0, "y": 100.0, "w": 100.0, "h": 24.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 24.0, "h": 24.0],
            ["op": "frame", "id": 3, "x": 30.0, "y": 0.0, "w": 40.0, "h": 24.0],
        ]))
        window.layoutIfNeeded()
        return p
    }

    private let slop: [String: Any] = ["hit_slop_top": 10, "hit_slop_right": 10, "hit_slop_bottom": 10, "hit_slop_left": 10]

    func testASlopTakesTouchesPastTheBoxAndMovesNothing() throws {
        let p = presenter(slop)
        let button = try XCTUnwrap(p.views[2])
        XCTAssertEqual(button.frame, CGRect(x: 0, y: 0, width: 24, height: 24), "layout never sees the slop")
        XCTAssertEqual(button.bounds, CGRect(x: 0, y: 0, width: 24, height: 24))
        for point in [CGPoint(x: 95, y: 112), CGPoint(x: 112, y: 92), CGPoint(x: 112, y: 132), CGPoint(x: 127, y: 112)] {
            XCTAssertTrue(window.hitTest(point, with: nil) === button, "in the slop: \(point)")
        }
        XCTAssertTrue(window.hitTest(CGPoint(x: 112, y: 112), with: nil) === button, "its box")
        XCTAssertFalse(window.hitTest(CGPoint(x: 88, y: 112), with: nil) === button, "past the slop")
        // The sibling paints over the slop where their boxes meet.
        XCTAssertTrue(window.hitTest(CGPoint(x: 132, y: 112), with: nil) === p.views[3])
        let plain = presenter([:])
        XCTAssertFalse(window.hitTest(CGPoint(x: 95, y: 112), with: nil) === plain.views[2], "no slop, no touch")
    }

    func testAClipCutsTheSlop() throws {
        let clipped = presenter(slop, clip: true)
        XCTAssertFalse(window.hitTest(CGPoint(x: 95, y: 112), with: nil) === clipped.views[2], "outside the clipping parent")
        XCTAssertTrue(window.hitTest(CGPoint(x: 127, y: 112), with: nil) === clipped.views[2], "inside it")
        var own = slop
        own["overflow_x"] = "hidden"; own["overflow_y"] = "hidden"
        let p = presenter(own)
        XCTAssertFalse(window.hitTest(CGPoint(x: 95, y: 112), with: nil) === p.views[2], "its own overflow clips its slop")
    }
}
#endif
