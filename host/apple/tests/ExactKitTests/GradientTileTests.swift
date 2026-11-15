import CoreGraphics
import XCTest
@testable import ExactKit

/// Repeating gradients as tiles (`GradientTile.swift`), held to the whole box
/// as `Gradient.paint` draws it, which is what a node's `draw(_:)` paints.
final class GradientTileTests: XCTestCase {
    /// Two faint diagonal hatches, as a sheet's page texture.
    static let hatch = ["repeating-linear-gradient(45deg, rgba(0,0,0,0.07) 0px 2px, transparent 2px 5px)",
                        "repeating-linear-gradient(-45deg, rgba(255,220,170,0.07) 0px 2px, transparent 2px 6px)"]
    static let grid = ["repeating-linear-gradient(0deg, #dfe7f1 0px 1px, transparent 1px 14px)",
                       "repeating-linear-gradient(90deg, #dfe7f1 0px 1px, transparent 1px 14px)"]
    /// Opaque stripes, where a stripe out of place shows most.
    static let contrast = "repeating-linear-gradient(30deg, #000 0px 3px, #fff 3px 7px)"

    static func gradient(_ css: String) -> Gradient { Gradient(["css": .string(css)] as BatchValue)! }

    static func pixels(_ image: CGImage) -> [UInt8] {
        let ctx = CGContext(data: nil, width: image.width, height: image.height, bitsPerComponent: 8, bytesPerRow: image.width * 4,
                            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        ctx.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
        return Array(UnsafeBufferPointer(start: ctx.data!.assumingMemoryBound(to: UInt8.self), count: image.width * image.height * 4))
    }

    /// The tiles as a node's layers place them over a box of `size`.
    static func tiled(_ period: Gradient.Period, size: CGSize, scale: CGFloat) -> CGImage {
        let rect = CGRect(origin: .zero, size: size), at = period.layout(box: rect, bounds: rect, scale: scale)!
        let (w, h) = (Int((size.width * scale).rounded(.up)), Int((size.height * scale).rounded(.up)))
        let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: 0,
                            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        for row in 0..<at.rows {
            for column in 0..<at.columns {
                let tile = period.image(at, key: period.key(at, column: column, row: row, scale: scale), scale: scale)!
                // Device pixels, y up.
                let x = at.origin.x * scale + CGFloat(column * at.width), top = at.origin.y * scale + CGFloat(row * at.height)
                ctx.draw(tile, in: CGRect(x: x, y: CGFloat(h) - top - CGFloat(at.height), width: CGFloat(at.width), height: CGFloat(at.height)))
            }
        }
        return ctx.makeImage()!
    }

    static func keys(_ period: Gradient.Period, size: CGSize, scale: CGFloat) -> Set<Int> {
        let rect = CGRect(origin: .zero, size: size), at = period.layout(box: rect, bounds: rect, scale: scale)!
        return Set((0..<at.rows).flatMap { row in (0..<at.columns).map { period.key(at, column: $0, row: row, scale: scale) } })
    }

    /// Per channel: the mean difference, and the share of pixels any of
    /// whose channels differs by more than `by`.
    static func difference(_ a: [UInt8], _ b: [UInt8], by: Int) -> (mean: Double, beyond: Double) {
        var (sum, beyond) = (0, 0)
        for p in stride(from: 0, to: a.count, by: 4) {
            var far = false
            for k in 0..<4 { let d = abs(Int(a[p + k]) - Int(b[p + k])); sum += d; far = far || d > by }
            beyond += far ? 1 : 0
        }
        return (Double(sum) / Double(a.count), Double(beyond) / Double(a.count / 4))
    }

    func testTexturesTakeTiles() {
        for css in Self.hatch + Self.grid + [Self.contrast] {
            XCTAssertNotNil(Self.gradient(css).period(dark: false), css)
        }
    }

    func testWhatATileCannotSayStaysDrawn() {
        for css in ["repeating-linear-gradient(45deg, red 0 10%, blue 10% 20%)",
                    "repeating-linear-gradient(90deg, red 0px 20px, blue 1% 30px)",
                    "repeating-linear-gradient(to top right, red 0 2px, blue 2px 5px)",
                    "linear-gradient(45deg, red, blue)"] {
            XCTAssertNil(Self.gradient(css).period(dark: false), css)
        }
    }

    /// Two heights a whole number of pixels apart, neither a whole number
    /// of periods, as a sheet's page has while it is dragged.
    func testTilesPaintWhatTheBoxDraws() {
        for scale: CGFloat in [2, 3] {
            for css in Self.hatch + Self.grid + [Self.contrast] {
                let gradient = Self.gradient(css), period = gradient.period(dark: false)!
                for size in [CGSize(width: 190, height: 150), CGSize(width: 190, height: 150 + 37 / scale)] {
                    let drawn = Self.pixels(gradient.image(size: size, scale: scale, dark: false)!)
                    let tiled = Self.pixels(Self.tiled(period, size: size, scale: scale))
                    let (mean, beyond) = Self.difference(drawn, tiled, by: 2)
                    print("tiles \(css) \(size.height) at \(scale)x: mean \(String(format: "%.4f", mean)), \(String(format: "%.3f", beyond * 100))% beyond 2")
                    // A tile's phase is a thirty-second of a pixel: an opaque
                    // edge moves by up to a sixty-fourth.
                    XCTAssertLessThan(mean, 0.5, "\(css) \(size) at \(scale)x")
                    XCTAssertLessThan(beyond, 0.01, "\(css) \(size) at \(scale)x")
                }
            }
        }
    }

    /// A height that moves the corner the stripes start from by whole
    /// pixels keeps the tiles' pictures: a sheet dragged shorter draws
    /// nothing.
    func testWholePixelResizeKeepsThePictures() {
        for scale: CGFloat in [2, 3] {
            for css in Self.hatch {
                let period = Self.gradient(css).period(dark: false)!, tall = CGSize(width: 402, height: 700)
                let before = Self.keys(period, size: tall, scale: scale)
                for pixels in 1...300 {
                    let after = Self.keys(period, size: CGSize(width: 402, height: 700 - CGFloat(pixels) / scale), scale: scale)
                    XCTAssert(after.isSubset(of: before), "\(css) -\(pixels) px at \(scale)x")
                }
            }
        }
    }
}
