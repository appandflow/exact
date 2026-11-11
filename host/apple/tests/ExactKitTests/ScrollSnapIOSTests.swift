#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1008's horizontal snap: `scroll-snap-align: start` rests an area's
/// start at the viewport's, `end` its end, and an area wider than the
/// viewport is free while it covers it (CSS Scroll Snap §5.2.2).
final class ScrollSnapIOSTests: XCTestCase {
    private func strip(_ aligns: [(String, Double, Double)], stop: String = "normal") throws -> (NodeView, UIScrollView) {
        let p = Presenter()
        p.viewport.frame = CGRect(x: 0, y: 0, width: 300, height: 100)
        var ops: [[String: Any]] = [["op": "create", "id": 1, "kind": "view", "style": ["overflow_x": "scroll", "scroll_snap_type": "x mandatory"]]]
        for (i, (align, x, w)) in aligns.enumerated() {
            ops.append(["op": "create", "id": 10 + i, "kind": "view", "style": ["scroll_snap_align": align, "scroll_snap_stop": stop]])
            ops.append(["op": "frame", "id": 10 + i, "x": x, "y": 0.0, "w": w, "h": 100.0])
        }
        ops += [["op": "children", "id": 1, "ids": aligns.indices.map { 10 + $0 }], ["op": "roots", "ids": [1]],
                ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 100.0],
                ["op": "content", "id": 1, "w": 900.0, "h": 100.0]]
        p.apply(wireBatch(ops))
        let node = try XCTUnwrap(p.views[1])
        return (node, try XCTUnwrap(node.scroll))
    }
    private func rest(_ node: NodeView, _ scroll: UIScrollView, from x: CGFloat, velocity: CGFloat = 0) -> CGFloat {
        var target = CGPoint(x: x, y: 0)
        node.scrollViewWillEndDragging(scroll, withVelocity: CGPoint(x: velocity, y: 0), targetContentOffset: &target)
        return target.x
    }

    func testEndRestsAnAreasEndAtTheViewportsEnd() throws {
        // A at 0 (start); B 300–450 (end: rests at 150); C 450–600 (start: 450).
        let (node, scroll) = try strip([("start", 0, 300), ("end", 300, 150), ("start", 450, 150)])
        XCTAssertEqual(rest(node, scroll, from: 60), 0)
        XCTAssertEqual(rest(node, scroll, from: 140), 150, "B's end at the viewport's end")
        XCTAssertEqual(rest(node, scroll, from: 400), 450)
    }

    func testAWideEndAreaIsFreeWhileItCoversTheViewport() throws {
        // 0–600, wider than the 300-point viewport: anywhere in 0…300.
        let (node, scroll) = try strip([("end", 0, 600), ("start", 600, 300)])
        XCTAssertEqual(rest(node, scroll, from: 120), 120)
        XCTAssertEqual(rest(node, scroll, from: 520), 600)
    }

    func testAlwaysStopsAFlingAtTheNextAreaInItsDirection() throws {
        let areas = (0..<6).map { ("start", Double($0) * 150, 150.0) }
        let (node, scroll) = try strip(areas, stop: "always")
        scroll.contentOffset.x = 20
        XCTAssertEqual(rest(node, scroll, from: 520), 150, "not past the next area's position")
        scroll.contentOffset.x = 440
        XCTAssertEqual(rest(node, scroll, from: 10), 300, "nor past the previous one's going back")
        scroll.contentOffset.x = 160
        XCTAssertEqual(rest(node, scroll, from: 170), 150, "a short release rests where it would")
        let (free, freeScroll) = try strip(areas)
        freeScroll.contentOffset.x = 20
        XCTAssertEqual(rest(free, freeScroll, from: 520), 450, "`normal` passes over positions")
    }

    func testAFlickReachesTheNextAreaHoweverShortItsTravel() throws {
        let (node, scroll) = try strip((0..<3).map { ("start", Double($0) * 300, 300.0) })
        scroll.contentOffset.x = 30
        XCTAssertEqual(rest(node, scroll, from: 90), 0, "a slow release returns")
        XCTAssertEqual(rest(node, scroll, from: 90, velocity: 0.8), 300)
        scroll.contentOffset.x = 280
        XCTAssertEqual(rest(node, scroll, from: 240, velocity: -0.8), 0)
        XCTAssertEqual(rest(node, scroll, from: 500, velocity: 0.8), 600, "a long fling still rests where it projects")
    }

    func testAgentRevealAndWheelRestAtAPagesSnapPosition() throws {
        let session = ExactApp.shared.makeSession(label: "snap-reveal")
        defer { session.destroy() }
        let p = session.presenter
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        defer { window.isHidden = true }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "style": ["overflow_x": "scroll", "overflow_y": "hidden", "scroll_snap_type": "x mandatory"]],
            ["op": "create", "id": 10, "kind": "view", "style": ["scroll_snap_align": "start"]],
            ["op": "create", "id": 11, "kind": "view", "style": ["scroll_snap_align": "start"]],
            ["op": "create", "id": 20, "kind": "button", "handlers": ["press"]],
            ["op": "children", "id": 1, "ids": [10, 11]], ["op": "children", "id": 11, "ids": [20]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 100, "w": 402, "h": 100],
            ["op": "frame", "id": 10, "x": 0, "y": 0, "w": 402, "h": 100],
            ["op": "frame", "id": 11, "x": 402, "y": 0, "w": 402, "h": 100],
            ["op": "frame", "id": 20, "x": 12, "y": 20, "w": 30, "h": 44],
            ["op": "content", "id": 1, "w": 804, "h": 100]
        ]))
        let scroll = try XCTUnwrap(p.views[1]?.scroll), agent = Agent(session: session)
        XCTAssertNil(agent.reveal(["id": 20])["error"])
        XCTAssertEqual(scroll.contentOffset.x, 402, accuracy: 0.01)
        scroll.contentOffset.x = 388
        XCTAssertNil(agent.reveal(["id": 20])["error"], "already visible, but between pages")
        XCTAssertEqual(scroll.contentOffset.x, 402, accuracy: 0.01)
        scroll.contentOffset.x = 0
        Agent.scroll(from: scroll, dx: 388, dy: 0)
        XCTAssertEqual(scroll.contentOffset.x, 402, accuracy: 0.01)
    }
}
#endif
