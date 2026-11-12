package dev.exact.canvas

import android.content.Context
import android.hardware.display.DisplayManager
import android.os.Build
import android.os.Handler
import android.os.HandlerThread
import android.view.Choreographer
import android.view.Display
import android.view.FrameMetrics
import android.view.Window
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicLong

/**
 * Choreographer's turns for [Owner] (LLP 1079.000 §6.2). On Android 13 and later a vsync callback, whose frame data
 * gives the frame's deadline and the spacing of the vsyncs it is scheduled on; before it a frame callback, which gives
 * neither. Both run in Choreographer's animation phase, so a turn is scheduled the same either way.
 */
internal class Turns(private val turned: (vsync: Long, deadline: Long, period: Long) -> Unit) {
    private val frame = Choreographer.FrameCallback { turned(it, 0L, 0L) }
    private val vsync: Any? = if (Build.VERSION.SDK_INT >= 33) Vsyncs.callback(turned) else null

    fun post(c: Choreographer) { if (vsync != null) Vsyncs.post(c, vsync) else c.postFrameCallback(frame) }
    fun remove(c: Choreographer) { if (vsync != null) Vsyncs.remove(c, vsync) else c.removeFrameCallback(frame) }
}

@android.annotation.TargetApi(33)
private object Vsyncs {
    fun callback(turned: (Long, Long, Long) -> Unit): Any = Choreographer.VsyncCallback { data ->
        // The timelines are the vsyncs this frame could be presented at, one period apart.
        val lines = data.frameTimelines
        val period = if (lines.size >= 2) lines[1].expectedPresentationTimeNanos - lines[0].expectedPresentationTimeNanos else 0L
        turned(data.frameTimeNanos, data.preferredFrameTimeline.deadlineNanos, period)
    }
    fun post(c: Choreographer, callback: Any) = c.postVsyncCallback(callback as Choreographer.VsyncCallback)
    fun remove(c: Choreographer, callback: Any) = c.removeVsyncCallback(callback as Choreographer.VsyncCallback)
}

/**
 * The display's refresh period, told at once and whenever its mode changes ([tell], on [handler]'s thread): a GPU
 * canvas's `Frame::period_ms` (LLP 1009) and the period of a turn Choreographer gives none.
 */
internal class DisplayPeriod(context: Context, private val handler: Handler, private val tell: (Long) -> Unit) :
    DisplayManager.DisplayListener {
    private val displays = context.getSystemService(DisplayManager::class.java)
    private var told = 0L

    init {
        displays.registerDisplayListener(this, handler)
        handler.post { check() }
    }

    override fun onDisplayChanged(id: Int) { if (id == Display.DEFAULT_DISPLAY) check() }
    override fun onDisplayAdded(id: Int) {}
    override fun onDisplayRemoved(id: Int) {}

    private fun check() {
        val hz = displays.getDisplay(Display.DEFAULT_DISPLAY)?.refreshRate ?: return
        if (!(hz > 0f)) return
        val period = Math.round(1e9 / hz)
        if (period != told) { told = period; tell(period) }
    }
}

/**
 * The window's [FrameMetrics] for `perf frames` (LLP 1079.000 §6.2, §5.1): each drawn frame's intended vsync, vsync,
 * total duration, deadline and GPU duration (the last two from Android 12; 0 before), copied on the listener's thread
 * into a fixed ring, since the platform reuses its object, and drained on the exact thread ([drain]). The listener
 * runs only while a driver's launch shows the activity ([attach]); a production bake collects nothing.
 */
internal class WindowFrames(private val wake: () -> Unit) {
    private val thread by lazy { HandlerThread("exact frame metrics").apply { start() } }
    private val ring = LongArray(RING * WORDS)
    /** Frames written (the listener's thread), published after each one's words. */
    private val written = AtomicLong(0)
    /** Reports the platform dropped before the listener ran. */
    private val dropped = AtomicLong(0)
    private val waking = AtomicBoolean(false)
    /** [read] as the listener's thread sees it. */
    private val readSeen = AtomicLong(0)
    /** Frames drained: the exact thread's. */
    private var read = 0L
    private var window: Window? = null

    private val listener = Window.OnFrameMetricsAvailableListener { _, m, drop ->
        record(
            m.getMetric(FrameMetrics.INTENDED_VSYNC_TIMESTAMP), m.getMetric(FrameMetrics.VSYNC_TIMESTAMP),
            m.getMetric(FrameMetrics.TOTAL_DURATION),
            if (Build.VERSION.SDK_INT >= 31) m.getMetric(FrameMetrics.DEADLINE) else 0L,
            if (Build.VERSION.SDK_INT >= 31) m.getMetric(FrameMetrics.GPU_DURATION) else 0L, drop,
        )
    }

    /** One frame's words, on the listener's thread, and the reports the platform dropped before it. */
    internal fun record(intended: Long, vsync: Long, total: Long, deadline: Long, gpu: Long, drop: Int) {
        val w = written.get()
        val at = ((w % RING) * WORDS).toInt()
        ring[at] = intended; ring[at + 1] = vsync; ring[at + 2] = total; ring[at + 3] = deadline; ring[at + 4] = gpu
        if (drop > 0) dropped.addAndGet(drop.toLong())
        written.set(w + 1)
        // Half the ring waits: the exact thread drains it before it laps, even with no turn of its own.
        if (w + 1 - readSeen.get() >= RING / 2 && !waking.getAndSet(true)) wake()
    }

    fun attach(window: Window) {
        if (this.window === window) return
        detach()
        this.window = window
        window.addOnFrameMetricsAvailableListener(listener, Handler(thread.looper))
    }

    fun detach() {
        val w = window ?: return
        window = null
        try { w.removeOnFrameMetricsAvailableListener(listener) } catch (e: IllegalArgumentException) {}
    }

    /** The exact thread: the frames written since the last drain into [into] ([WORDS] each), their count, and in
     * [lost] the reports dropped by the platform or overwritten here before they were read. */
    fun drain(into: LongArray, lost: LongArray): Int {
        waking.set(false)
        val w = written.get()
        var from = read
        if (w - from > RING) { lost[0] += w - from - RING; from = w - RING }
        val n = (w - from).toInt()
        for (k in 0 until n) System.arraycopy(ring, (((from + k) % RING) * WORDS).toInt(), into, k * WORDS, WORDS)
        // Frames the listener wrote over while they were copied are torn: dropped, and counted.
        val torn = (written.get() - RING - from).coerceIn(0L, n.toLong()).toInt()
        if (torn > 0) {
            System.arraycopy(into, torn * WORDS, into, 0, (n - torn) * WORDS)
            lost[0] += torn.toLong()
        }
        read = w
        readSeen.set(w)
        lost[0] += dropped.getAndSet(0)
        return n - torn
    }

    companion object {
        const val RING = 64
        const val WORDS = 5
    }
}
