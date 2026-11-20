// CSS `mix-blend-mode` on a box (CSS Compositing 1 §3), through AppKit's
// public `NSView.compositingFilter`. Core Animation blends the view with
// everything drawn behind it in the window, not only up to the box's
// stacking context. iOS has no public route (LLP 1077 D9).
#if os(macOS)
import AppKit
import CoreImage

extension NodeView {
    func applyBoxBlend() {
        let filters = [
            "multiply": "CIMultiplyBlendMode", "screen": "CIScreenBlendMode", "overlay": "CIOverlayBlendMode",
            "darken": "CIDarkenBlendMode", "lighten": "CILightenBlendMode", "color-dodge": "CIColorDodgeBlendMode",
            "color-burn": "CIColorBurnBlendMode", "hard-light": "CIHardLightBlendMode", "soft-light": "CISoftLightBlendMode",
            "difference": "CIDifferenceBlendMode", "exclusion": "CIExclusionBlendMode", "hue": "CIHueBlendMode",
            "saturation": "CISaturationBlendMode", "color": "CIColorBlendMode", "luminosity": "CILuminosityBlendMode",
        ]
        let name = style["mix_blend_mode"]?.string.flatMap { filters[$0] }
        guard compositingFilter?.name != name else { return }
        compositingFilter = name.flatMap { CIFilter(name: $0) }
    }
}
#endif
