import XCTest
import CoreGraphics
@testable import ExactKit
#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// CSS `background-blend-mode`: a box's gradient layers blend with its
/// background colour, as one group over what is behind the box.
final class BackgroundBlendTests: XCTestCase {
    private var session: ExactSession?
    override func tearDown() { session?.destroy(); session = nil; super.tearDown() }

    private static let hatch = "repeating-linear-gradient(45deg, #cccccc 0 10px, #ffffff 10px 20px)"
    private static let base: [Int] = [42, 122, 226]

    private func box(_ style: [String: Any]) throws -> NodeView {
        let session = ExactApp.shared.makeSession(label: "background-blend")
        self.session = session
        session.presenter.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "style": style],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 80.0, "h": 80.0],
        ]))
        return try XCTUnwrap(session.presenter.views[1])
    }

    /// The box's pixels over an opaque `backdrop`, as its layer tree draws them.
    private func pixels(_ node: NodeView, over backdrop: CGColor = CGColor(gray: 0, alpha: 1)) throws -> [[Int]] {
        #if os(macOS)
        node.display()
        let layer = try XCTUnwrap(node.layer)
        #else
        node.setNeedsDisplay()
        node.layer.displayIfNeeded()
        let layer = node.layer
        #endif
        let ctx = try XCTUnwrap(CGContext(data: nil, width: 80, height: 80, bitsPerComponent: 8, bytesPerRow: 320,
            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        ctx.setFillColor(backdrop); ctx.fill(CGRect(x: 0, y: 0, width: 80, height: 80))
        layer.render(in: ctx)
        let bytes = try XCTUnwrap(ctx.data).assumingMemoryBound(to: UInt8.self)
        return stride(from: 0, to: 80 * 80 * 4, by: 4).map { i in (0..<3).map { Int(bytes[i + $0]) } }
    }

    /// The share of `pixels` within `tolerance` of `rgb` in every channel.
    private func share(_ pixels: [[Int]], _ rgb: [Int], tolerance: Int = 6) -> Double {
        let near = pixels.filter { p in zip(p, rgb).allSatisfy { abs($0 - $1) <= tolerance } }
        return Double(near.count) / Double(pixels.count)
    }

    private func style(_ mode: String?) -> [String: Any] {
        var style: [String: Any] = ["background_color": Self.base + [255], "background_image": ["css": Self.hatch]]
        if let mode { style["background_blend_mode"] = mode }
        return style
    }

    func testAMultipliedStripeDarkensTheColourBeneathIt() throws {
        let node = try box(style("multiply"))
        XCTAssertNotNil(node.backgroundBlend)
        XCTAssertTrue(node.gradientDraws, "a blended layer draws, never tiled over the layer's colour")
        let shot = try pixels(node)
        let stripe = Self.base.map { Int((Double($0) * 204 / 255).rounded()) }
        XCTAssertGreaterThan(share(shot, stripe), 0.25, "the grey stripes are the colour, darker: \(stripe)")
        XCTAssertGreaterThan(share(shot, Self.base), 0.25, "the white stripes leave the colour")
        XCTAssertEqual(share(shot, [204, 204, 204]), 0, "no stripe draws unblended")
    }

    /// One plain gradient, which Core Animation would otherwise show as a gradient layer.
    func testAMultipliedLinearGradientDraws() throws {
        var linear = style("multiply")
        linear["background_image"] = ["css": "linear-gradient(to right, #ffffff, #ffffff)"]
        let node = try box(linear)
        XCTAssertTrue(node.gradientDraws)
        XCTAssertGreaterThan(share(try pixels(node), Self.base), 0.9, "white multiplied leaves the colour")
    }

    func testNormalPaintsTheStripesOverTheColour() throws {
        let node = try box(style(nil))
        XCTAssertNil(node.backgroundBlend)
        let shot = try pixels(node)
        XCTAssertGreaterThan(share(shot, [204, 204, 204]), 0.25)
        XCTAssertGreaterThan(share(shot, [255, 255, 255]), 0.25)
    }

    /// The group is the box's own: over a transparent colour the stripes
    /// show as they are, not multiplied with the red behind the box.
    func testTheLayersDoNotBlendWithWhatIsBehindTheBox() throws {
        var clear = style("multiply")
        clear["background_color"] = [0, 0, 0, 0]
        let shot = try pixels(try box(clear), over: CGColor(red: 1, green: 0, blue: 0, alpha: 1))
        XCTAssertGreaterThan(share(shot, [204, 204, 204]), 0.25)
        XCTAssertGreaterThan(share(shot, [255, 255, 255]), 0.25)
    }

    #if os(macOS)
    /// A box's `mix-blend-mode` is AppKit's compositing filter, cleared by `normal`.
    func testABoxBlendIsItsCompositingFilter() throws {
        let node = try box(["background_color": [255, 214, 0, 255], "mix_blend_mode": "multiply"])
        XCTAssertEqual(node.compositingFilter?.name, "CIMultiplyBlendMode")
        node.applyStyle(["background_color": [255, 214, 0, 255], "mix_blend_mode": "normal"])
        XCTAssertNil(node.compositingFilter)
    }
    #endif
}
