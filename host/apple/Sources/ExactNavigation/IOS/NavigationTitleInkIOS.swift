#if os(iOS) || os(tvOS)
import UIKit
import ExactKit

/// Item appearances keep an authored title colour and font local to its route.
final class NavigationTitleInk {
    /// The fonts a heading's authored typography gives the bar's inline and
    /// large titles; nil where it gives that title none (`fonts(of:level:traits:)`).
    struct Fonts: Equatable {
        var title: UIFont?, large: UIFont?
    }

    private var saved: [UINavigationBarAppearance?]?
    private var projected: [UINavigationBarAppearance?]?
    private var ink: UIColor?
    private var fonts: Fonts?
    private var transparent = false
    private var pair: (light: CGColor, dark: CGColor, color: UIColor)?

    /// The colour `heading`'s author wrote for the bar's title. A `light-dark()` pair on a heading
    /// that takes the surrounding appearance is a colour UIKit resolves in the bar's appearance: an
    /// appearance change restyles no node, and the heading's own traits may not have turned yet
    /// when the bar is prepared for it. One colour for each pair, so an unchanged pair is no change.
    func color(of heading: NodeView?) -> UIColor? {
        guard let heading, let own = heading.ownUIColor("text_color") else { return nil }
        guard heading.colorScheme == .unspecified, heading.style["text_color"]?.tintAlpha == nil,
              let light = heading.cgColor("text_color", dark: false), let dark = heading.cgColor("text_color", dark: true),
              light != dark else { return own }
        if let pair, pair.light == light, pair.dark == dark { return pair.color }
        let color = UIColor { $0.userInterfaceStyle == .dark ? UIColor(cgColor: dark) : UIColor(cgColor: light) }
        pair = (light, dark, color)
        return color
    }

    #if os(iOS)
    /// UIKit's own title fonts, which a row the author left unsaid keeps: the
    /// inline title's `headline` (17-point semibold at the default size) and
    /// the large title's bold `largeTitle` (34 points), each in `traits`
    /// (Dynamic Type, Bold Text).
    static func platformFonts(_ traits: UITraitCollection) -> (title: UIFont, large: UIFont) {
        let large = UIFont.preferredFont(forTextStyle: .largeTitle, compatibleWith: traits)
        let bold = large.fontDescriptor.withSymbolicTraits(.traitBold).map { UIFont(descriptor: $0, size: 0) } ?? large
        return (UIFont.preferredFont(forTextStyle: .headline, compatibleWith: traits), bold)
    }

    /// What `heading`'s authored typography gives the bar's titles. Its size
    /// is the title's it is shown as: the large title's for a level-1 heading
    /// (its collapsed inline title keeps UIKit's size), else the inline one's.
    /// Nil when its author wrote none of it, which leaves UIKit's fonts.
    static func fonts(of heading: NodeView?, level: Int, traits: UITraitCollection) -> Fonts? {
        // Read on every projection: most headings write none, and are answered before UIKit's fonts are made.
        guard let heading, ["font_family", "font_weight", "font_style", "font_size"].contains(where: heading.wrote) else { return nil }
        let platform = platformFonts(traits)
        let fonts = Fonts(title: heading.authoredTitleFont(over: platform.title, sized: level != 1),
                          large: heading.authoredTitleFont(over: platform.large, sized: level == 1))
        return fonts.title == nil && fonts.large == nil ? nil : fonts
    }
    #endif

    func update(_ color: UIColor?, fonts: Fonts? = nil, item: UINavigationItem, bar: UINavigationBar?, transparent: Bool) {
        let current = [item.standardAppearance, item.compactAppearance, item.scrollEdgeAppearance, item.compactScrollEdgeAppearance]
        if color == ink, fonts == self.fonts, transparent == self.transparent, current == projected { return }
        ink = color
        self.fonts = fonts
        self.transparent = transparent
        let styled = color != nil || fonts != nil
        if saved == nil, styled || transparent { saved = current }
        guard let saved else { return }
        let base = bar?.standardAppearance ?? UINavigationBarAppearance()
        func painted(_ appearance: UINavigationBarAppearance?) -> UINavigationBarAppearance? {
            guard styled else { return appearance }
            let copy = (appearance ?? base).copy() as! UINavigationBarAppearance
            if let color { copy.titleTextAttributes[.foregroundColor] = color }
            if let font = fonts?.title { copy.titleTextAttributes[.font] = font }
            #if !os(tvOS)
            if let color { copy.largeTitleTextAttributes[.foregroundColor] = color }
            if let font = fonts?.large { copy.largeTitleTextAttributes[.font] = font }
            #endif
            return copy
        }
        let next = [
            painted(saved[0]),
            painted(styled ? saved[1] ?? bar?.compactAppearance : saved[1]),
            painted(transparent ? NavigationHost.clearScrollEdge : styled ? saved[2] ?? bar?.scrollEdgeAppearance : saved[2]),
            painted(transparent ? NavigationHost.clearScrollEdge : styled ? saved[3] ?? bar?.compactScrollEdgeAppearance : saved[3])
        ]
        item.standardAppearance = next[0]
        item.compactAppearance = next[1]
        item.scrollEdgeAppearance = next[2]
        item.compactScrollEdgeAppearance = next[3]
        projected = next
        if !styled, !transparent { self.saved = nil }
    }
}
#endif
