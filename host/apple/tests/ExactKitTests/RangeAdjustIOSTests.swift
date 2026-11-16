#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// VoiceOver's swipe up and down on a drawn `slider` and `spinbutton`
/// (`range-adjust.contract`, compiled and run): each is adjustable, and an
/// increment or decrement is the keydown and keyup WebKit sends an ARIA range,
/// which the author's `key` handler answers by changing `aria-valuenow`.
/// UIKit, so a simulator runs it: bun host/apple/build.mjs --test --ios
final class RangeAdjustIOSTests: XCTestCase {
    private var session: ExactSession?
    private var window: UIWindow?

    override func tearDown() {
        session?.destroy(); session = nil
        window?.isHidden = true; window = nil
        super.tearDown()
    }

    private func booted() throws -> ExactSession {
        let path = try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_RANGE_ADJUST_PLAN"])
        let session = ExactApp.shared.makeSession(label: "range-adjust")
        self.session = session
        let host = UIViewController(), view = ExactView(session: session)
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        self.window = window
        XCTAssertNil(session.boot(plan: try Data(contentsOf: URL(fileURLWithPath: path)), size: window.bounds.size).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        return session
    }

    private func node(_ name: String) throws -> NodeView {
        try XCTUnwrap(session?.presenter.views.values.first { $0.props["testId"] == name }, name)
    }

    func testIncrementAndDecrementRunTheAuthorsKeyHandler() throws {
        _ = try booted()
        let level = try node("level"), floor = try node("floor")
        XCTAssertTrue(level.accessibilityTraits.contains(.adjustable))
        XCTAssertTrue(floor.accessibilityTraits.contains(.adjustable))
        XCTAssertEqual(level.accessibilityValue, "50 percent")
        level.accessibilityIncrement()
        waitFor("the slider's ArrowRight") { level.props["accessibilityValueNow"] == "60" }
        XCTAssertEqual(level.accessibilityValue, "60 percent")
        level.accessibilityDecrement()
        level.accessibilityDecrement()
        waitFor("the slider's two ArrowLefts") { level.props["accessibilityValueNow"] == "40" }
        floor.accessibilityIncrement()
        waitFor("the spin button's ArrowUp") { floor.accessibilityValue == "4" }
        floor.accessibilityDecrement()
        waitFor("the spin button's ArrowDown") { floor.accessibilityValue == "3" }
    }

    /// A horizontal slider's increment follows its own computed `direction`:
    /// ArrowLeft for one that inherits `rtl` in a left-to-right document, and
    /// ArrowRight for one that says `ltr` under it.
    func testASlidersIncrementFollowsItsOwnDirection() throws {
        _ = try booted()
        let back = try node("back"), ahead = try node("ahead")
        back.accessibilityIncrement()
        waitFor("the inherited rtl slider's ArrowLeft") { back.props["accessibilityValueNow"] == "60" }
        ahead.accessibilityIncrement()
        waitFor("the ltr slider's ArrowRight") { ahead.props["accessibilityValueNow"] == "60" }
    }
}
#endif
