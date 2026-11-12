// Shared by both Apple hosts; the iOS runner selects *IOSTests by name.
#if os(macOS)
import AppKit
#else
import UIKit
#endif
import XCTest
@testable import ExactKit

/// `scripts/fixtures/layout-insert.contract`: a row inserted by `if` above a
/// sibling that slides under its layout transition (LLP 1063).
///   bun host/apple/build.mjs --test --ios --only LayoutInsertIOSTests
///   bun host/apple/build.mjs --test --only LayoutInsertIOSTests
final class LayoutInsertIOSTests: XCTestCase {
    #if os(macOS)
    private var window: NSWindow?
    #else
    private var window: UIWindow?
    #endif
    private var session: ExactSession?

    override func tearDown() {
        session?.destroy()
        session = nil
        #if os(macOS)
        window?.close()
        #else
        window?.isHidden = true
        #endif
        window = nil
        super.tearDown()
    }

    private func boot() throws -> ExactSession {
        let plan = try fixturePlan("LAYOUT_INSERT")
        let session = ExactApp.shared.makeSession(label: "layout-insert")
        self.session = session
        let view = ExactView(session: session)
        #if os(macOS)
        _ = NSApplication.shared
        let size = NSSize(width: 402, height: 874)
        let window = NSWindow(contentRect: NSRect(origin: .zero, size: size), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = view
        self.window = window
        XCTAssertNil(session.boot(plan: plan, size: size).error)
        view.layoutSubtreeIfNeeded()
        #else
        let scene = try XCTUnwrap(hostedWindowScene(), "a window of the test host's scene")
        let host = UIViewController()
        let window = UIWindow(windowScene: scene)
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        self.window = window
        XCTAssertNil(session.boot(plan: plan, size: window.bounds.size).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        #endif
        waitFor("the page is laid out") { self.node(session, "below")?.window != nil }
        return session
    }

    private func node(_ session: ExactSession, _ testId: String) -> NodeView? {
        session.presenter.views.values.first { $0.props["testId"] == testId }
    }

    /// Where layout put `v` in its parent: on iOS from its centre, which its transform
    /// leaves alone (its frame does not); AppKit moves only the layer.
    private func laidY(_ v: NodeView) -> CGFloat {
        #if os(macOS)
        v.frame.minY
        #else
        v.center.y - v.bounds.height / 2
        #endif
    }
    /// Where `v` shows: there, moved by the layout transition's offset.
    private func shownY(_ v: NodeView) -> CGFloat { laidY(v) + v.layoutOffset.y }

    #if os(iOS)
    /// The slider is measured before the commit's frame shows: when the tap's batch
    /// has applied, with no turn of the run loop between, the row is laid out at the
    /// slider's own height and the sibling, laid out below it, still shows where it
    /// was. Measured a turn later, the sibling showed one frame where the default
    /// height put it, then the start, then moved (#134). AppKit's slider is the
    /// default's height, so only UIKit's is measured taller.
    func testASliderInsertedAboveAMovingSiblingIsMeasuredBeforeItsFrame() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "the slider is measured in a scene's window") }
        let session = try boot()
        let below = try XCTUnwrap(node(session, "below"))
        let was = laidY(below)
        session.presenter.onPress?(try XCTUnwrap(node(session, "toggle")).id)
        let row = try XCTUnwrap(node(session, "row")), box = try XCTUnwrap(node(session, "slider"))
        let slider = try XCTUnwrap(session.presenter.controls.controls[box.id])
        XCTAssertGreaterThan(slider.frame.height, 16, "the platform's slider, taller than the default it is laid out at first")
        XCTAssertEqual(box.bounds.height, slider.frame.height, accuracy: 0.5, "the input is laid out at the slider's own height")
        XCTAssertEqual(row.frame.maxY, laidY(below), accuracy: 0.5, "the sibling is laid out below the measured row")
        XCTAssertEqual(shownY(below), was, accuracy: 0.5, "and still shows where it was")
        waitFor("the sibling slides to its place") { below.layoutOffset == .zero }
    }
    #endif
}
