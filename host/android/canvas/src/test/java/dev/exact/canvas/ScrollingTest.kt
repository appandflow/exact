package dev.exact.canvas

import android.app.Activity
import android.view.MotionEvent
import android.widget.FrameLayout
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [31], manifest = Config.NONE)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class ScrollingTest {
    private class Fixture(val axis: Int) {
        val activity = Robolectric.buildActivity(Activity::class.java).get()
        val view = ExactView(activity, false) {}
        val parent = FrameLayout(activity).also { it.addView(view) }
        val scrolling = Scrolling(view)
        val scrollers = Scrolling::class.java.getDeclaredField("scrollers").apply { isAccessible = true }
        val indicate = Scrolling::class.java.getDeclaredMethod("indicate", Scrolling.Scroller::class.java).apply { isAccessible = true }
        fun scroller() = (scrollers.get(scrolling) as Map<*, *>)[1] as Scrolling.Scroller
        fun told(flags: Int) = scrolling.told(intArrayOf(1, flags, 0, 0, 0, 0, 0, 500f.toBits(), 500f.toBits()))
        fun down() {
            val e = MotionEvent.obtain(0, 0, MotionEvent.ACTION_DOWN, 100f, 100f, 0)
            scrolling.touch(e); e.recycle()
        }
        fun reply(policy: Int) = intArrayOf(0, 1, 0, 1, axis or policy, 0, 0, 0,
            500f.toBits(), 500f.toBits(), 0, 0, 200f.toBits(), 200f.toBits())
        fun under(reply: IntArray) = scrolling.under(scrolling.serial, reply)
        fun indicate() = indicate.invoke(scrolling, scroller())
    }

    @Test fun hitPolicyIsAFallbackUntilStatementsArrive() {
        for (axis in listOf(1, 2)) {
            val f = Fixture(axis)
            f.down()
            f.under(f.reply(0))
            f.indicate()
            assertEquals(2, f.parent.childCount)
            f.down()
            f.under(f.reply(8))
            assertEquals("none in a hit reply removes an indicator before any statement", 1, f.parent.childCount)
            f.indicate()
            assertEquals(1, f.parent.childCount)
            f.told(axis)
            f.indicate()
            assertEquals(2, f.parent.childCount)
            f.told(axis or 8)
            assertEquals("a live none removes the existing indicator", 1, f.parent.childCount)
            f.told(axis)
            f.indicate()
            assertEquals(2, f.parent.childCount)
        }
    }

    @Test fun anOldAutoReplyCannotOverrideANewerNoneStatement() = oldReply(0, 8)

    @Test fun anOldNoneReplyCannotOverrideANewerAutoStatement() = oldReply(8, 0)

    private fun oldReply(oldPolicy: Int, newPolicy: Int) {
        for (axis in listOf(1, 2)) {
            val f = Fixture(axis)
            f.told(axis or oldPolicy)
            f.down()
            val old = f.reply(oldPolicy)
            f.told(axis or newPolicy)
            f.under(old)
            f.indicate()
            assertEquals("the statement's policy survives an older hit reply", newPolicy != 0, f.scroller().barless)
            assertEquals(if (newPolicy == 0) 2 else 1, f.parent.childCount)
        }
    }

    @Test fun aTouchsAnswerTellsEachScrollersSnap() {
        val f = Fixture(1)
        f.down()
        val module = IntArray(8)
        // Sideways, mandatory: a 200 snapport, two areas, the second an `always` one; nothing up and down.
        val snap = intArrayOf(1 or 4, 200f.toBits(), 2, 0, 0, 0, 300f.toBits(), 300f.toBits(), 1, 0, 0)
        f.under(f.reply(0) + module + snap)
        val s = f.scroller()
        assertTrue(s.snapMandatory)
        assertEquals(200f, s.snapPort[0], 0f)
        assertArrayEquals(floatArrayOf(0f, 0f, 300f, 300f), s.snaps[0], 0f)
        assertArrayEquals(booleanArrayOf(false, true), s.stops[0])
        assertEquals(0f, s.snapPort[1], 0f)
        assertEquals(0, s.snaps[1].size)
    }

    @Test fun aSnapRestsAtTheNearestPositionShortOfAnAlwaysOne() {
        val snaps = floatArrayOf(0f, 0f, 250f, 250f, 550f, 550f, 800f, 800f)
        val normal = BooleanArray(4)
        assertEquals(550f, snapRest(snaps, normal, 600f, 250f, true, 400f)!!, 0f)
        assertEquals("the smaller of two as near", 250f, snapRest(snaps, normal, 400f, 250f, true, 400f)!!, 0f)
        val always = booleanArrayOf(true, true, true, true)
        assertEquals("not past the next `always` position", 550f, snapRest(snaps, always, 780f, 250f, true, 400f)!!, 0f)
        assertEquals("nor the previous one going back", 250f, snapRest(snaps, always, 10f, 550f, true, 400f)!!, 0f)
        assertEquals(800f, snapRest(snaps, booleanArrayOf(false, false, false, true), 780f, 250f, true, 400f)!!, 0f)
        // An area longer than the snapport rests anywhere it covers it, and an `always` one is left at its end.
        val long = floatArrayOf(0f, 0f, 300f, 900f, 1200f, 1200f)
        assertEquals(640f, snapRest(long, BooleanArray(3), 640f, 400f, true, 400f)!!, 0f)
        assertEquals(900f, snapRest(long, booleanArrayOf(false, true, false), 1150f, 400f, true, 400f)!!, 0f)
        // `proximity`: only within a third of the snapport of where it would rest.
        assertEquals(250f, snapRest(snaps, normal, 380f, 250f, false, 400f)!!, 0f)
        assertNull(snapRest(snaps, normal, 400f, 250f, false, 400f))
        assertNull(snapRest(FloatArray(0), BooleanArray(0), 400f, 250f, true, 400f))
    }
}
