#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactNavigation

/// A bar title's `light-dark()` ink in a route that writes no `color-scheme`
/// follows the session's appearance as it changes: nothing in the app reads
/// the scheme, so no batch restyles the heading.
final class NavigationTitleSchemeIOSTests: XCTestCase {
    private var window: UIWindow?
    private var host: UIViewController?
    private var sessions: [ExactSession] = []

    override func tearDown() {
        for session in sessions { session.destroy() }
        sessions = []
        window?.isHidden = true
        window = nil
        host = nil
        super.tearDown()
    }

    private func until(_ what: String, _ seconds: Double = 5, _ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(seconds)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
        XCTAssertTrue(done(), what)
    }

    func testASheetTitleFollowsTheAppearanceItsRouteInherits() throws {
        guard let scene = hostedWindowScene() else {
            throw missing("scene", "UIKit presents a sheet only from a window a scene holds")
        }
        let session = ExactApp.shared.makeSession(label: "sheet-title-follow")
        sessions.append(session)
        let view = ExactView(session: session)
        let host = UIViewController()
        let window = UIWindow(windowScene: scene)
        window.frame = window.screen.bounds
        window.overrideUserInterfaceStyle = .light
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        self.window = window
        self.host = host
        XCTAssertNil(session.boot(plan: try fixturePlan("SHEET_TITLE_FOLLOW"), size: window.bounds.size).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        until("the session is light") { session.schemeDark == false }
        // A sheet waits for the first drawn frame, which this window never draws.
        if session.firstDrawMs == nil { session.firstDrawMs = ExactEnv.wall() }
        let p = session.presenter
        p.press(try XCTUnwrap(p.views.values.first { $0.props["testId"] == "open" }).id)
        until("the sheet is presented") { host.presentedViewController != nil }
        let route = try XCTUnwrap(p.views.values.first { $0.props["testId"] == "route-sheet" })
        let controller = try XCTUnwrap(p.navigationHost.controllers[route.id])
        func labels(_ view: UIView) -> [UILabel] { ((view as? UILabel).map { [$0] } ?? []) + view.subviews.flatMap(labels) }
        // The red channel of the title's ink as the bar draws it: 0 in light, 1 in dark.
        let red = { () -> CGFloat? in
            guard let bar = controller.navigationController?.navigationBar else { return nil }
            bar.layoutIfNeeded()
            guard let label = labels(bar).first(where: { $0.text == "Sheet" }) else { return nil }
            let ink = (label.attributedText?.attribute(.foregroundColor, at: 0, effectiveRange: nil) as? UIColor) ?? label.textColor
            var r: CGFloat = 0
            ink?.resolvedColor(with: label.traitCollection).getRed(&r, green: nil, blue: nil, alpha: nil)
            return r
        }
        until("the title is drawn in the light ink") { red().map { $0 < 0.1 } == true }
        for (style, dark) in [(UIUserInterfaceStyle.dark, true), (.light, false)] {
            window.overrideUserInterfaceStyle = style
            until("the session follows the window") { session.schemeDark == dark }
            until("the title is drawn in the \(dark ? "dark" : "light") ink") { red().map { dark ? $0 > 0.9 : $0 < 0.1 } == true }
        }
    }
}
#endif
