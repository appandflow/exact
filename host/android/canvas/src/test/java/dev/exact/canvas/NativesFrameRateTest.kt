package dev.exact.canvas

import android.view.Surface
import android.view.SurfaceView
import android.view.View
import java.io.File
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import org.robolectric.annotation.Implementation
import org.robolectric.annotation.Implements
import org.robolectric.shadow.api.Shadow
import org.robolectric.shadows.ShadowNativeSurface

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [31], manifest = Config.NONE, shadows = [NativesFrameRateTest.Votes::class])
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class NativesFrameRateTest {
    @Implements(Surface::class)
    class Votes : ShadowNativeSurface() {
        val votes = ArrayList<Pair<Float, Int>>()
        var valid = true
        @Implementation fun isValid() = valid
        @Implementation fun setFrameRate(fps: Float, compatibility: Int) { votes.add(fps to compatibility) }
    }

    private fun natives() = Natives(RuntimeEnvironment.getApplication(), { File(".") }, emptyMap()).apply {
        ensure(7, 3, 0, 0, "{}", 0)
    }

    /** Supply a compositor surface without starting the Rust GPU host. */
    private fun surface(natives: Natives): Votes {
        val elements = Natives::class.java.getDeclaredField("elements").apply { isAccessible = true }
            .get(natives) as Map<*, *>
        val e = elements[7]!!
        val surface = Surface(android.graphics.SurfaceTexture(0))
        e.javaClass.getDeclaredField("view").apply { isAccessible = true }
            .set(e, SurfaceView(natives.context))
        e.javaClass.getDeclaredField("surface").apply { isAccessible = true }.set(e, surface)
        return Shadow.extract(surface)
    }

    @Test fun aCanvasCapVotesDefaultToSuppressGlobalTouchBoostAndZeroClearsIt() {
        val natives = natives()
        val surface = surface(natives)
        // AOSP's ExplicitDefault vote suppresses global touch boost; ExactOrMultiple does not (#574).
        for (fps in listOf(24, 30, 60, 90, 120, 0)) {
            natives.frameRates(intArrayOf(7, fps))
            assertEquals(fps.toFloat() to Surface.FRAME_RATE_COMPATIBILITY_DEFAULT, surface.votes.last())
        }
        assertEquals(6, surface.votes.size)
        surface.valid = false
        natives.frameRates(intArrayOf(7, 60))
        assertEquals("a destroyed surface gets no vote", 6, surface.votes.size)
    }

    @Test @Config(sdk = [35]) fun aCanvasInTheReaderWindowKeepsItsNumericViewVote() {
        val natives = natives()
        val view = natives.getChildAt(0)
        natives.frameRates(intArrayOf(7, 60))
        assertEquals(60f, view.requestedFrameRate, 0f)
        natives.frameRates(intArrayOf(7, 0))
        assertEquals(View.REQUESTED_FRAME_RATE_CATEGORY_DEFAULT, view.requestedFrameRate, 0f)
    }
}
