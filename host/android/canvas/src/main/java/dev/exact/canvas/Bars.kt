package dev.exact.canvas

import android.graphics.fonts.Font
import android.view.Gravity
import android.view.View
import android.widget.FrameLayout
import java.nio.IntBuffer

/**
 * The app's bars: a route's `header` as Material 3's top app bar and the navigation root's
 * `tablist` as its navigation bar. What counts as either is exact2's to say (the kernel's rule, which the iOS host
 * reads too), and so is the picture: each bar is painted by exact2 from Material's own metrics and replayed here in a
 * [Pane], a view of its own over the page, as a surface of the top layer is in a window of its own ([Surfaces]).
 * No library, and nothing of the framework's widgets.
 *
 * Over the page, not in it: the top bar is at the window's top and fills the status bar's area with its own colour;
 * the navigation bar is at the window's bottom and fills the gesture bar's. The page is where exact2 puts it
 * ([Native.chrome]), inset by the room the bars take. A keyboard ends the page above it and leaves the navigation
 * bar where it is, under the keyboard.
 *
 * exact2 says what the bars are with each stream ([Native.bars], [Owner.Pub.bars]), when that is not what it last
 * said, so a bar and the page under it change in one frame: which bar, its frame in this window in pixels, how long
 * a finger rests on it before its pan begins ([Hold]), and its picture. A bar it leaves out is gone. A touch in a bar
 * goes back by one call ([Exact.bar]) at the bar's own pixels; exact2 knows what is there.
 * `host/linux/src/presenter/bars.rs` is exact2's side.
 *
 * A statement often arrives while the page's view is being drawn, and a bar it shows or moves must be in that same
 * frame, or the page is shown for a frame without its bar. So both views are made with the reader, in a layer of
 * their own over the page that is always there (the window's size, drawing nothing itself, taking no touch), and
 * each is laid out by hand when exact2 says where it is. A layout asked of the parent would come a frame late; and
 * the layer, which the frame's drawing reaches after the page's view, records its bars again in that same pass,
 * where a view shown straight in the reader's root would wait for the root to be recorded again.
 */
internal class Bars(activity: ExactActivity, root: FrameLayout) {
    /** The faces the bars have drawn with, by what announces each: a bar painted again makes none anew. */
    private val faces = HashMap<List<Int>, Font>()
    private val layer = FrameLayout(activity).also {
        // A bar is a picture: what is in it is said by the reader's root ([Access]).
        it.importantForAccessibility = View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS
        root.addView(it, FrameLayout.LayoutParams(FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.MATCH_PARENT))
    }
    private val slop = android.view.ViewConfiguration.get(activity).scaledTouchSlop
    private val holds = arrayOf(Hold(TOP), Hold(BOTTOM))
    private val panes = arrayOf(pane(activity, TOP), pane(activity, BOTTOM))

    private fun pane(activity: ExactActivity, which: Int): Pane {
        val holds = holds[which - 1]
        val pane = Pane(activity, faces, { action, x, y -> if (!holds.touch(action, x, y)) Exact.bar(which, action, x, y) }, null)
        pane.visibility = View.INVISIBLE
        layer.addView(pane, FrameLayout.LayoutParams(0, 0).apply { gravity = Gravity.TOP or Gravity.LEFT })
        return pane
    }

    /** exact2's statement of its bars ([Native.bars]). */
    fun told(w: IntArray) {
        val shown = BooleanArray(panes.size)
        var at = 1
        repeat(w[0]) {
            val which = w[at]; val n = w[at + 6]
            val pane = panes.getOrNull(which - 1)
            if (pane != null) {
                shown[which - 1] = true
                holds[which - 1].ms = w[at + 5]
                place(pane, w[at + 1], w[at + 2], w[at + 3], w[at + 4])
                pane.show(IntBuffer.wrap(w, at + 7, n).slice())
                if (pane.visibility != View.VISIBLE) pane.visibility = View.VISIBLE
            }
            at += 7 + n
        }
        for ((i, pane) in panes.withIndex()) if (!shown[i] && pane.visibility == View.VISIBLE) pane.visibility = View.INVISIBLE
    }

    /** Put [pane] at its frame now, and leave its layout parameters saying the same for the parent's next pass. */
    private fun place(pane: Pane, x: Int, y: Int, width: Int, height: Int) {
        if (pane.left == x && pane.top == y && pane.width == width && pane.height == height) return
        (pane.layoutParams as FrameLayout.LayoutParams).let { lp ->
            lp.width = width; lp.height = height; lp.leftMargin = x; lp.topMargin = y
        }
        pane.measure(
            View.MeasureSpec.makeMeasureSpec(width, View.MeasureSpec.EXACTLY),
            View.MeasureSpec.makeMeasureSpec(height, View.MeasureSpec.EXACTLY),
        )
        pane.layout(x, y, x + width, y + height)
    }

    /**
     * `-exact-pan-hold` on the bar's own node (the tablist's `pan`, holding the navigation bar): a finger that rests
     * [ms] within the touch slop begins the pan (4), which the same finger then moves (2) and releases with its
     * velocity in pixels a second (6), or cancels (3); the bar's target lets go. A touch that moves or lifts first is
     * the bar's as before. Whether a touch is held is the reader's to time, as Android's long press is the view's.
     */
    private inner class Hold(private val which: Int) {
        var ms = 0
        private var held = false
        private var downX = 0f
        private var downY = 0f
        private val samples = ArrayDeque<FloatArray>()
        private val handler = android.os.Handler(android.os.Looper.getMainLooper())
        private val begin = Runnable {
            held = true
            Exact.bar(which, 4, downX, downY)
        }

        /** Whether the touch is the hold's, not the bar's. */
        fun touch(action: Int, x: Float, y: Float): Boolean {
            if (held) {
                sample(x, y)
                when (action) {
                    2 -> Exact.bar(which, 2, x, y)
                    1 -> {
                        Exact.bar(which, 2, x, y)
                        val v = velocity()
                        Exact.bar(which, 6, v[0], v[1])
                        held = false
                    }
                    else -> { Exact.bar(which, 3, x, y); held = false }
                }
                return true
            }
            when (action) {
                0 -> if (ms > 0) {
                    downX = x; downY = y; samples.clear(); sample(x, y)
                    handler.postDelayed(begin, ms.toLong())
                }
                2 -> if (Math.hypot((x - downX).toDouble(), (y - downY).toDouble()) > slop) handler.removeCallbacks(begin)
                else -> handler.removeCallbacks(begin)
            }
            return false
        }

        private fun sample(x: Float, y: Float) {
            samples.addLast(floatArrayOf(android.os.SystemClock.uptimeMillis().toFloat(), x, y))
            if (samples.size > 12) samples.removeFirst()
        }

        /** Pixels a second over the last 80 ms of the touch, as the iOS host measures its hold's release. */
        private fun velocity(): FloatArray {
            val end = samples.lastOrNull() ?: return floatArrayOf(0f, 0f)
            val start = samples.firstOrNull { end[0] - it[0] <= 80f } ?: return floatArrayOf(0f, 0f)
            val dt = (end[0] - start[0]) / 1000f
            if (dt <= 0.001f) return floatArrayOf(0f, 0f)
            return floatArrayOf((end[1] - start[1]) / dt, (end[2] - start[2]) / dt)
        }
    }

    companion object {
        /** Which bar (`presenter::bars`): the top app bar, the navigation bar. */
        const val TOP = 1
        const val BOTTOM = 2
    }
}
