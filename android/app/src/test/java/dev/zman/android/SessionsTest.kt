package dev.zman.android

import org.junit.Assert.assertEquals
import org.junit.Test

class SessionsTest {
    private val s = 1_000L
    private fun resumed(pkg: String, at: Long) = RawEvent(pkg, Sessions.RESUMED, at * s)
    private fun paused(pkg: String, at: Long) = RawEvent(pkg, Sessions.PAUSED, at * s)

    @Test
    fun switchingAppsClosesThePreviousSession() {
        val sessions = Sessions.build(
            listOf(resumed("a", 0), paused("a", 60), resumed("b", 60), paused("b", 90)),
            now = 500 * s,
        )
        assertEquals(listOf(Session("a", 0, 60 * s), Session("b", 60 * s, 90 * s)), sessions)
    }

    @Test
    fun movingBetweenScreensOfOneAppIsOneSession() {
        val sessions = Sessions.build(
            listOf(resumed("a", 0), paused("a", 10), resumed("a", 11), paused("a", 40)),
            now = 500 * s,
        )
        assertEquals(listOf(Session("a", 0, 40 * s)), sessions)
    }

    @Test
    fun comingBackLaterIsANewSession() {
        val sessions = Sessions.build(
            listOf(resumed("a", 0), paused("a", 10), resumed("a", 300), paused("a", 320)),
            now = 500 * s,
        )
        assertEquals(listOf(Session("a", 0, 10 * s), Session("a", 300 * s, 320 * s)), sessions)
    }

    @Test
    fun screenOffEndsTheSession() {
        val sessions = Sessions.build(
            listOf(resumed("a", 0), RawEvent("android", Sessions.SCREEN_OFF, 30 * s)),
            now = 5_000 * s,
        )
        assertEquals(listOf(Session("a", 0, 30 * s)), sessions)
    }

    @Test
    fun appStillInFrontRunsUntilNow() {
        val sessions = Sessions.build(listOf(resumed("a", 0)), now = 45 * s)
        assertEquals(listOf(Session("a", 0, 45 * s)), sessions)
    }

    @Test
    fun launcherAndBlinksAreDropped() {
        val events = listOf(
            resumed("launcher", 0), paused("launcher", 20),
            resumed("a", 20), RawEvent("a", Sessions.PAUSED, 20 * s + 300),
            resumed("b", 21), paused("b", 50),
        )
        val sessions = Sessions.build(events, now = 500 * s, ignore = setOf("launcher"))
        assertEquals(listOf(Session("b", 21 * s, 50 * s)), sessions)
    }

    @Test
    fun idsAreStableAndDistinct() {
        val a = Session("com.a", 1_000, 2_000)
        val grown = Session("com.a", 1_000, 9_000)
        val other = Session("com.b", 1_000, 2_000)
        assertEquals(Sessions.id("0123456789abcdef", a), Sessions.id("0123456789abcdef", grown))
        assert(Sessions.id("0123456789abcdef", a) != Sessions.id("0123456789abcdef", other))
        assert(Sessions.id("0123456789abcdef", a).matches(Regex("[A-Za-z0-9_-]{1,64}")))
    }
}
