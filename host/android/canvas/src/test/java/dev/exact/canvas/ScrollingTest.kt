package dev.exact.canvas

import org.junit.Assert.assertEquals
import org.junit.Test

/** What a drag that exact2's answer let start late takes of the way the finger already went, and where a snap stops. */
class ScrollingTest {
    @Test fun aLateDragTakesTheWayPastTheSlopEitherWay() {
        assertEquals(112f, pastSlop(120f, 8f), 0f)
        assertEquals(-112f, pastSlop(-120f, 8f), 0f)
    }

    @Test fun withinTheSlopNothingMoves() {
        assertEquals(0f, pastSlop(5f, 8f), 0f)
        assertEquals(0f, pastSlop(-8f, 8f), 0f)
    }

    @Test fun aFlingStopsAtTheFirstAlwaysAreaEitherWay() {
        val snaps = floatArrayOf(0f, 0f, 250f, 250f, 550f, 550f, 800f, 800f)
        val always = booleanArrayOf(true, true, true, true)
        assertEquals(550f, stopAt(250f, 800f, snaps, always), 0f)
        assertEquals(250f, stopAt(550f, 0f, snaps, always), 0f)
        assertEquals(250f, stopAt(250f, 250f, snaps, always), 0f)
        assertEquals(800f, stopAt(250f, 800f, snaps, booleanArrayOf(false, false, false, true)), 0f)
    }
}
