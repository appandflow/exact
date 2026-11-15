// A repeating linear gradient as tiles of device pixels. Its stripes run
// across the box at one angle, so the picture repeats along each axis after
// the period divided by that axis's share of the gradient line: tiles about
// that size fall at nearly the same place in a period, and those that fall
// at the same place share one picture.
import CoreGraphics
import Foundation

extension Gradient {
    /// One period of a repeating linear gradient whose stops are all
    /// lengths, so it is the same period at every box size.
    struct Period {
        /// The resolved gradient: its colour space.
        fileprivate let ramp: Gradient
        fileprivate let dark: Bool
        /// The gradient line's unit direction, y down.
        let direction: CGPoint
        /// Points along the line.
        let length: CGFloat
        /// Where a period begins, in points along the line from the
        /// corner the line starts at (CSS places the line's start on the
        /// perpendicular through that corner), 0 up to `length`.
        let offset: CGFloat
        /// One period's stops, 0 to 1 of it.
        let locations: [CGFloat]
        let colors: [CGColor]
    }

    /// The period, when the gradient is a repeating linear one whose
    /// period and place along the line do not change with the box: no
    /// percentage stop and no `to <corner>`, which it checks by resolving
    /// at two sizes.
    func period(dark: Bool) -> Period? {
        guard css?.hasPrefix("repeating-linear-gradient(") == true else { return nil }
        let boxes = [CGRect(x: 0, y: 0, width: 1000, height: 1000), CGRect(x: 0, y: 0, width: 1037, height: 1291)]
        let periods = boxes.compactMap { box in resolved(in: box, dark: dark)?.cycle(in: box, dark: dark) }
        guard periods.count == 2, periods[0].matches(periods[1]) else { return nil }
        return periods[0]
    }

    /// The resolved gradient's whole periods, laid end to end from the
    /// line's start (`placed.rs`), as one period.
    private func cycle(in box: CGRect, dark: Bool) -> Period? {
        guard case .angle = shape, !linear(dark: dark), case .axial(let a, let b) = placement(in: box) else { return nil }
        let (locations, colors) = stops(dark: dark)
        let span = hypot(b.x - a.x, b.y - a.y), n = locations.count
        guard n >= 2, span > 0 else { return nil }
        // Premultiplying can add a stop where one period meets the next,
        // which the line's start has none of, so the stop count need not
        // divide evenly: a period is the shortest run from the first stop
        // that the rest repeats, at least twice over.
        guard let count = (1...(n / 2)).first(where: { q in
            let d = locations[q] - locations[0]
            return d > 0 && (q..<n).allSatisfy { colors[$0] == colors[$0 - q] && abs(locations[$0] - locations[$0 - q] - d) < 1e-5 }
        }) else { return nil }
        let d = locations[count] - locations[0]
        var direction = CGPoint(x: (b.x - a.x) / span, y: (b.y - a.y) / span)
        // The kernel's line is single precision: a right angle it misses by
        // a hair would otherwise repeat along that axis only after millions
        // of pixels.
        if abs(direction.x) < 1e-5 { direction = CGPoint(x: 0, y: direction.y < 0 ? -1 : 1) }
        if abs(direction.y) < 1e-5 { direction = CGPoint(x: direction.x < 0 ? -1 : 1, y: 0) }
        let length = d * span
        let corner = Gradient.startCorner(of: box, direction: direction)
        let along = (a.x - corner.x) * direction.x + (a.y - corner.y) * direction.y
        let offset = along - (along / length).rounded(.down) * length
        return Period(ramp: self, dark: dark, direction: direction, length: length, offset: offset,
                      locations: locations[..<count].map { ($0 - locations[0]) / d }, colors: Array(colors[..<count]))
    }

    /// The corner of `box` the gradient line starts across: the least far
    /// along `direction`.
    static func startCorner(of box: CGRect, direction u: CGPoint) -> CGPoint {
        CGPoint(x: u.x > 0 ? box.minX : box.maxX, y: u.y > 0 ? box.minY : box.maxY)
    }
}

extension Gradient.Period {
    fileprivate func matches(_ other: Gradient.Period) -> Bool {
        let tolerance: CGFloat = 1e-3
        var shift = abs(offset - other.offset)
        shift = min(shift, length - shift)
        return abs(length - other.length) < tolerance && shift < tolerance
            && abs(direction.x - other.direction.x) < 1e-6 && abs(direction.y - other.direction.y) < 1e-6
            && locations.count == other.locations.count && colors == other.colors
            && zip(locations, other.locations).allSatisfy { abs($0 - $1) < 1e-4 }
    }

    /// A tile's device pixels along each axis: the whole number of pixels
    /// up to 512 that comes closest to whole periods, so that tiles next to
    /// each other fall at the same place in a period and share a picture;
    /// 256 along an axis the stripes run along, or one too long to repeat
    /// within 512.
    func tile(scale: CGFloat) -> (width: Int, height: Int) {
        func side(_ share: CGFloat) -> Int {
            let limit = 512
            let repeats = share == 0 ? .infinity : length * scale / abs(share)
            guard repeats <= CGFloat(limit) else { return 256 }
            var best = (pixels: 256, error: CGFloat.infinity)
            for k in 1...Int(CGFloat(limit) / repeats) {
                let exact = repeats * CGFloat(k), pixels = max(1, Int(exact.rounded()))
                let error = abs(exact - CGFloat(pixels)) / CGFloat(pixels)
                if error < best.error - 1e-12 { best = (pixels, error) }
            }
            // Fewer, larger tiles where a small one repeats exactly.
            return best.pixels * max(1, (best.error < 1e-6 ? 256 : 1) / best.pixels)
        }
        return (side(direction.x), side(direction.y))
    }

    /// Tiles over a layer's `bounds` for a gradient box `box`.
    struct Layout: Equatable {
        /// One tile's device pixels.
        let width: Int, height: Int
        /// The first tile's top left, from the bounds' top left: at most a
        /// tile up and left of it, in whole device pixels.
        let origin: CGPoint
        let columns: Int, rows: Int
        /// How far along the line the first tile's top left is past where
        /// a period begins, and how much further each column and row is,
        /// in points.
        let phase: CGFloat, column: CGFloat, row: CGFloat
    }

    /// The tiles start from the device pixel of the corner the line starts
    /// at, so a size that moves that corner by whole pixels moves them and
    /// leaves each tile's picture as it was.
    func layout(box: CGRect, bounds: CGRect, scale: CGFloat) -> Layout? {
        guard bounds.width > 0, bounds.height > 0 else { return nil }
        let (w, h) = tile(scale: scale), (tw, th) = (CGFloat(w), CGFloat(h)), u = direction
        let anchor = Gradient.startCorner(of: box, direction: u)
        let ax = ((anchor.x - bounds.minX) * scale).rounded(), ay = ((anchor.y - bounds.minY) * scale).rounded()
        let gx = ax - tw * (ax / tw).rounded(.up), gy = ay - th * (ay / th).rounded(.up)
        let along = (bounds.minX + gx / scale - anchor.x) * u.x + (bounds.minY + gy / scale - anchor.y) * u.y - offset
        return Layout(width: w, height: h, origin: CGPoint(x: gx / scale, y: gy / scale),
                      columns: Int(((bounds.width * scale - gx) / tw).rounded(.up)), rows: Int(((bounds.height * scale - gy) / th).rounded(.up)),
                      phase: along, column: tw / scale * u.x, row: th / scale * u.y)
    }

    /// The picture for the tile at `column`, `row`: its phase in
    /// thirty-seconds of a device pixel, which tiles that fall at the same
    /// place in a period share.
    func key(_ layout: Layout, column: Int, row: Int, scale: CGFloat) -> Int {
        let along = layout.phase + layout.column * CGFloat(column) + layout.row * CGFloat(row)
        let phase = along - (along / length).rounded(.down) * length
        return Int((phase * scale * 32).rounded())
    }

    /// The tile picture for `key` (`key(_:column:row:scale:)`).
    func image(_ layout: Layout, key: Int, scale: CGFloat) -> CGImage? {
        image(width: layout.width, height: layout.height, scale: scale, phase: CGFloat(key) / (scale * 32))
    }

    /// A tile `width` × `height` device pixels at `scale` whose top left
    /// is `phase` points along the line from where a period begins, drawn
    /// as `Gradient.paint` draws the whole box.
    func image(width: Int, height: Int, scale: CGFloat, phase: CGFloat) -> CGImage? {
        guard let ctx = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0,
                                  space: CGColorSpace(name: CGColorSpace.sRGB)!,
                                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
        // y down, as the view's backing store.
        ctx.translateBy(x: 0, y: CGFloat(height)); ctx.scaleBy(x: scale, y: -scale)
        let rect = CGRect(x: 0, y: 0, width: CGFloat(width) / scale, height: CGFloat(height) / scale)
        let u = direction, origin = CGPoint(x: -u.x * phase, y: -u.y * phase)
        let along = [CGPoint(x: rect.minX, y: rect.minY), CGPoint(x: rect.maxX, y: rect.minY), CGPoint(x: rect.minX, y: rect.maxY), CGPoint(x: rect.maxX, y: rect.maxY)]
            .map { (($0.x - origin.x) * u.x + ($0.y - origin.y) * u.y) / length }
        let first = Int(along.min()!.rounded(.down)) - 1, last = Int(along.max()!.rounded(.up)) + 1, count = last - first
        var stops: [CGFloat] = [], colors: [CGColor] = []
        for c in 0..<count {
            stops += locations.map { (CGFloat(c) + $0) / CGFloat(count) }
            colors += self.colors
        }
        func point(_ periods: Int) -> CGPoint { CGPoint(x: origin.x + u.x * length * CGFloat(periods), y: origin.y + u.y * length * CGFloat(periods)) }
        ctx.addRect(rect); ctx.clip()
        ramp.paintAxial(ctx, start: point(first), end: point(last), locations: stops, colors: colors, dark: dark)
        return ctx.makeImage()
    }
}
