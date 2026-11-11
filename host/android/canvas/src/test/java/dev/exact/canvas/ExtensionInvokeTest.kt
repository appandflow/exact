package dev.exact.canvas

import java.util.concurrent.Executors
import org.junit.Assert.*
import org.junit.Test

/** JVM doubles for the owner and JNI boundary; Exact's dispatch runs unchanged. */
class Owner : AutoCloseable {
    private val thread = Executors.newSingleThreadExecutor()
    var publications = 0
        private set
    fun <T> sync(publish: Boolean = true, f: () -> T): T = thread.submit<T> {
        val answer = f()
        if (publish) publications++
        answer
    }.get()
    override fun close() { thread.shutdownNow() }
}

object Native {
    private val host = ThreadLocal<String>()
    var invokeAvailable = true
    var received: String? = null
    var onChanged: (String) -> Unit = {}
    @JvmStatic fun changed(topic: String) = onChanged(topic)
    @JvmStatic fun underAgent(name: String) = false
    @JvmStatic fun appFile(path: String): String? = null
    fun start() { host.set("running") }
    fun stop() { host.remove(); received = null }
    @JvmStatic fun invoke(body: String): Int {
        if (!invokeAvailable) throw UnsatisfiedLinkError("Native.invoke")
        if (host.get() == null) return -1
        received = body
        return 37
    }
}

class ExtensionInvokeTest {
    @Test fun aMissingNativeEntryCanBeAnsweredAsNoSessionOnEitherThread() {
        Owner().use { owner ->
            Exact.owner = null
            Native.invokeAvailable = false
            try {
                assertThrows(UnsatisfiedLinkError::class.java) { Exact.invoke("main-thread host") }
                Exact.owner = owner
                assertEquals(-1, Exact.invoke("owner-thread host"))
            } finally {
                Native.invokeAvailable = true
                Exact.owner = null
            }
        }
    }

    @Test fun invokeReachesTheOwnerAndPublishesTheHandlersChanges() {
        Owner().use { owner ->
            Exact.owner = owner
            try {
                owner.sync(publish = false) { Native.start() }
                assertEquals(-1, Native.invoke("wrong thread"))
                assertEquals(37, Exact.invoke("handler arguments"))
                assertEquals("handler arguments", Native.received)
                assertEquals(1, owner.publications)
            } finally {
                owner.sync(publish = false) { Native.stop() }
                Exact.owner = null
            }
        }
    }

    @Test fun invokeAlsoWorksWhenTheHostRunsOnTheCallingThread() {
        Exact.owner = null
        Native.start()
        var publications = 0
        Exact.turned = { publications++ }
        try {
            assertEquals(37, Exact.invoke("main-thread host"))
            assertEquals(1, publications)
            assertEquals("main-thread host", Native.received)
        } finally { Native.stop(); Exact.turned = {} }
        assertEquals(-1, Exact.invoke("no host"))
    }
}
