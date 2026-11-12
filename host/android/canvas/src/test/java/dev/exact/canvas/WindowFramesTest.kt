package dev.exact.canvas

import org.junit.Assert.*
import org.junit.Test

class WindowFramesTest {
    private val words = LongArray(WindowFrames.RING * WindowFrames.WORDS)

    @Test fun framesDrainInOrderOnceAndTheDroppedAreCounted() {
        var wakes = 0
        val frames = WindowFrames { wakes++ }
        frames.record(100, 110, 6_000_000, 8_000_000, 2_000_000, 0)
        frames.record(200, 210, 9_000_000, 8_000_000, 3_000_000, 2)
        val lost = LongArray(1)
        assertEquals(2, frames.drain(words, lost))
        assertEquals(listOf(100L, 110L, 6_000_000L, 8_000_000L, 2_000_000L), words.take(5))
        assertEquals(200L, words[5])
        assertEquals("the platform's dropped reports", 2L, lost[0])
        lost[0] = 0
        assertEquals("each frame once", 0, frames.drain(words, lost))
        assertEquals(0L, lost[0])
        assertEquals("under half the ring asks for no drain", 0, wakes)
    }

    @Test fun halfARingWakesTheExactThreadAndALappedRingKeepsTheNewest() {
        var wakes = 0
        val frames = WindowFrames { wakes++ }
        for (k in 0 until WindowFrames.RING / 2) frames.record(k.toLong() + 1, 0, 0, 0, 0, 0)
        assertEquals(1, wakes)
        // Once asked, it is not asked again until a drain
        for (k in WindowFrames.RING / 2 until WindowFrames.RING + 10) frames.record(k.toLong() + 1, 0, 0, 0, 0, 0)
        assertEquals(1, wakes)
        val lost = LongArray(1)
        val n = frames.drain(words, lost)
        assertEquals(WindowFrames.RING, n)
        assertEquals("the ten it overwrote", 10L, lost[0])
        assertEquals("the oldest kept is the eleventh", 11L, words[0])
        assertEquals((WindowFrames.RING + 10).toLong(), words[(n - 1) * WindowFrames.WORDS])
    }
}
