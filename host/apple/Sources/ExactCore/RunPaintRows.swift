import Foundation

package struct RunPaintRows: Equatable {
    /// Each shadow's offset x, y and blur in points, the first on top;
    /// empty is `none`.
    package var shadows: [[Double]] = []
    /// Each shadow's colour; nil is `currentcolor`.
    package var shadowColors: [BatchValue?] = []
    package var strokeWidth: Double = 0
    /// The stroke's colour; nil is `currentcolor`.
    package var strokeColor: BatchValue?
    /// `paint-order` paints the stroke before, so under, the fill.
    package var strokeUnder = false

    package init() {}
    /// Whether a colour here is a `light-dark()` pair.
    package var paired: Bool { shadowColors.contains { $0?.isSchemeColor == true } || strokeColor?.isSchemeColor == true }

    /// `[{"o": [x, y], "b": blur, "c": colour}, …]`, as the host crosses it.
    package mutating func readShadow(_ value: BatchValue) {
        shadows = []; shadowColors = []
        for item in value.array ?? [] {
            guard case .object(let o) = item, let offset = o["o"]?.numbers, offset.count == 2 else { continue }
            shadows.append(offset + [max(0, o["b"]?.number ?? 0)])
            shadowColors.append(o["c"])
        }
    }

    /// Whether `paint-order`'s CSS text names the stroke before the fill.
    package static func strokesFirst(_ order: String) -> Bool {
        let words = order.split(separator: " ")
        guard let stroke = words.firstIndex(of: "stroke") else { return false }
        return words.firstIndex(of: "fill").map { stroke < $0 } ?? true
    }
}
