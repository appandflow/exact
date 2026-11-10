package dev.exact.canvas

import org.junit.Assert.assertEquals
import org.junit.Test

/** What a drag that exact2's answer let start late takes of the way the finger already went. */
class ScrollingTest {
    @Test fun aLateDragTakesTheWayPastTheSlopEitherWay() {
        assertEquals(112f, pastSlop(120f, 8f), 0f)
        assertEquals(-112f, pastSlop(-120f, 8f), 0f)
    }

    @Test fun withinTheSlopNothingMoves() {
        assertEquals(0f, pastSlop(5f, 8f), 0f)
        assertEquals(0f, pastSlop(-8f, 8f), 0f)
    }
}
