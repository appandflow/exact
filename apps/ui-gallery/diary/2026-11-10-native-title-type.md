# UI Gallery: the native bar's title type

## Rough

A section heading wrote `font-size=(authoredHeader ? 22 : 34) font-weight=600` so that the header the page draws looked like a large title. Since the Apple host carries an authored heading font into the native title, that line made iOS draw the large title at 34-point semibold: lighter than UIKit's bold, and fixed at 34 whatever the reader's Dynamic Type size. A ternary cannot leave an attribute unwritten, so the heading is now two `if` branches.

## Lean in

Under a native bar (iOS, the Mac, where `host` is `native` and the header is not authored) the heading writes no type, and its font rows are the compiler's heading rows, not authored ones (`layout` shows `font_size = 28 (heading)`), so UIKit draws its own bold large title. The web and the authored header keep the 34 and 22 point semibold they had. `sectionTitle` names each section once.

## Needed

Nothing.

## Checkpoints

iOS 27.0 simulator, `--chrome platform`: the Text colors large title was semibold before and is UIKit's bold now, matching the root's "UI Gallery" title. Web: the heading is 34px at weight 600 before and after, at the first frame and after the data arrives. macOS: the window title is "Typography" before and after. Under the driver's own chrome, the Mac's painted heading is now the platform's level-1 heading (22 points) rather than 34-point semibold.

`text.test.contract` on the iPhone simulator fails its two native tests with and without this change, alike: the first view each taps is not on screen, because the iPhone shows the section list (route 0) where the tests expect a section. On the web it passes 2, skips 1.
