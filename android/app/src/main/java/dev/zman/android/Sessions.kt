package dev.zman.android

/** One thing Android reported: an app came to the front, left it, or the screen went off. */
data class RawEvent(val pkg: String, val type: Int, val time: Long)

/** A stretch of time with one app in the foreground. Times are epoch milliseconds. */
data class Session(val pkg: String, val start: Long, var end: Long) {
    val durationMs: Long get() = end - start
}

/**
 * Turns Android's usage events into sessions. Plain Kotlin with no Android
 * classes, so it can be unit tested on the JVM.
 */
object Sessions {
    // Values of android.app.usage.UsageEvents.Event constants.
    const val RESUMED = 1
    const val PAUSED = 2
    const val SCREEN_OFF = 16
    const val KEYGUARD_SHOWN = 17

    /** Moving between two screens of the same app pauses and resumes it within moments. */
    private const val SAME_APP_GAP_MS = 2_000L
    private const val MIN_SESSION_MS = 1_000L

    /**
     * @param events usage events, in any order
     * @param now end given to a session still in the foreground
     * @param ignore packages never recorded (the home screen)
     */
    fun build(events: List<RawEvent>, now: Long, ignore: Set<String> = emptySet()): List<Session> {
        val sessions = mutableListOf<Session>()
        var current: Session? = null
        var open = false

        for (event in events.sortedBy { it.time }) {
            when (event.type) {
                RESUMED -> {
                    val c = current
                    val sameApp = c != null && c.pkg == event.pkg &&
                        (open || event.time - c.end <= SAME_APP_GAP_MS)
                    if (!sameApp) {
                        if (c != null) {
                            if (open) c.end = event.time
                            sessions += c
                        }
                        current = Session(event.pkg, event.time, event.time)
                    }
                    open = true
                }
                PAUSED -> {
                    val c = current
                    if (c != null && open && c.pkg == event.pkg) {
                        c.end = event.time
                        open = false
                    }
                }
                SCREEN_OFF, KEYGUARD_SHOWN -> {
                    val c = current
                    if (c != null && open) {
                        c.end = event.time
                        open = false
                    }
                }
            }
        }
        current?.let {
            if (open) it.end = maxOf(now, it.start)
            sessions += it
        }
        return sessions.filter { it.pkg !in ignore && it.durationMs >= MIN_SESSION_MS }
    }

    /**
     * A stable id for a session: reading the same usage events again yields
     * the same id, so re-uploading updates the event instead of duplicating it.
     */
    fun id(deviceId: String, session: Session): String {
        val pkgHash = Integer.toHexString(session.pkg.hashCode())
        return "${deviceId.take(8)}-${session.start}-$pkgHash"
    }
}
