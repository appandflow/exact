#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A node whose background is repeating gradients paints them as tiles
/// (`GradientTilesIOS.swift`) and keeps no bitmap of its own.
final class GradientTilesIOSTests: XCTestCase {
    private let ground: [Double] = [251, 246, 238, 255]

    private func node(size: CGSize, scale: CGFloat) -> NodeView {
        let p = Presenter()
        let node = NodeView(id: 1, kind: "view", presenter: p)
        p.views[node.id] = node
        let window = UIWindow(frame: CGRect(origin: .zero, size: CGSize(width: 400, height: 800)))
        window.addSubview(node)
        node.frame = CGRect(origin: .zero, size: size)
        node.contentScaleFactor = scale
        node.applyStyle(["background_color": .array(ground.map { .number($0) }),
                         "background_image": .array(GradientTileTests.hatch.map { ["css": .string($0)] as BatchValue })])
        node.layer.displayIfNeeded()
        return node
    }

    /// The node's layers, as Core Animation's `render(in:)` composites them.
    private func rendered(_ node: NodeView, scale: CGFloat) -> [UInt8] {
        let (w, h) = (Int(node.bounds.width * scale), Int(node.bounds.height * scale))
        let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        ctx.translateBy(x: 0, y: CGFloat(h)); ctx.scaleBy(x: scale, y: -scale)
        node.layer.render(in: ctx)
        return GradientTileTests.pixels(ctx.makeImage()!)
    }

    /// The box as `draw(_:)` paints it: the colour, then each gradient, the last first.
    private func drawn(size: CGSize, scale: CGFloat) -> [UInt8] {
        let (w, h) = (Int(size.width * scale), Int(size.height * scale))
        let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        ctx.translateBy(x: 0, y: CGFloat(h)); ctx.scaleBy(x: scale, y: -scale)
        let rect = CGRect(origin: .zero, size: size)
        ctx.setFillColor(CGColor(srgbRed: ground[0] / 255, green: ground[1] / 255, blue: ground[2] / 255, alpha: 1)); ctx.fill(rect)
        for css in GradientTileTests.hatch.reversed() {
            GradientTileTests.gradient(css).paint(ctx, clip: CGPath(rect: rect, transform: nil), box: rect, dark: false)
        }
        return GradientTileTests.pixels(ctx.makeImage()!)
    }

    func testTexturedBoxKeepsNoBitmapAndResizesWithoutDrawing() {
        let scale: CGFloat = 3, node = node(size: CGSize(width: 200, height: 300), scale: scale)
        XCTAssertFalse(node.drawsPaint)
        XCTAssertNil(node.layer.contents)
        XCTAssertNotNil(node.gradientTiles?.layer)
        var (mean, beyond) = GradientTileTests.difference(rendered(node, scale: scale), drawn(size: node.bounds.size, scale: scale), by: 2)
        XCTAssertLessThan(mean, 0.5); XCTAssertLessThan(beyond, 0.01)

        // Shorter by whole pixels: every tile shows a picture it already had.
        let before = Set((node.gradientTiles?.pictures ?? []).flatMap { $0.images.values.map(ObjectIdentifier.init) })
        node.frame.size.height = 300 - 41 / scale
        node.layer.displayIfNeeded()
        XCTAssertNil(node.layer.contents)
        let shown = (node.gradientTiles?.layer?.sublayers ?? []).flatMap { $0.sublayers ?? [] }.compactMap { $0.contents.map { ObjectIdentifier($0 as AnyObject) } }
        XCTAssertFalse(shown.isEmpty)
        XCTAssert(Set(shown).isSubset(of: before), "a resize drew a tile")
        (mean, beyond) = GradientTileTests.difference(rendered(node, scale: scale), drawn(size: node.bounds.size, scale: scale), by: 2)
        XCTAssertLessThan(mean, 0.5); XCTAssertLessThan(beyond, 0.01)
    }

    /// A dashed border draws the box, gradients and all.
    func testDrawnBoxPaintsItsGradients() {
        let node = node(size: CGSize(width: 120, height: 90), scale: 2)
        var style = node.style
        for side in ["top", "right", "bottom", "left"] {
            style["border_width_" + side] = .number(2); style["border_style_" + side] = .string("dashed")
            style["border_color_" + side] = .array([0.0, 0, 0, 255].map { .number($0) })
        }
        node.applyStyle(style)
        node.layer.displayIfNeeded()
        XCTAssertTrue(node.drawsPaint)
        XCTAssertNil(node.gradientTiles?.layer)
    }
}
#endif
