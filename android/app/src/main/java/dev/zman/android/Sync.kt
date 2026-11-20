package dev.zman.android

import android.app.AppOpsManager
import android.app.usage.UsageEvents
import android.app.usage.UsageStatsManager
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Process
import org.json.JSONArray
import org.json.JSONObject
import java.net.HttpURLConnection
import java.net.URL
import java.util.UUID

/** Account and cursor, kept in the app's private storage. */
class Prefs(context: Context) {
    private val prefs = context.getSharedPreferences("zman", Context.MODE_PRIVATE)

    var server: String?
        get() = prefs.getString("server", null)
        set(value) = prefs.edit().putString("server", value).apply()
    var email: String?
        get() = prefs.getString("email", null)
        set(value) = prefs.edit().putString("email", value).apply()
    var token: String?
        get() = prefs.getString("token", null)
        set(value) = prefs.edit().putString("token", value).apply()

    /** End of the last period of usage events already uploaded. */
    var cursor: Long
        get() = prefs.getLong("cursor", 0)
        set(value) = prefs.edit().putLong("cursor", value).apply()
    var lastSync: Long
        get() = prefs.getLong("last_sync", 0)
        set(value) = prefs.edit().putLong("last_sync", value).apply()

    /** Identifies this installation; created once. */
    val deviceId: String
        get() = prefs.getString("device_id", null) ?: UUID.randomUUID().toString().replace("-", "").also {
            prefs.edit().putString("device_id", it).apply()
        }

    val signedIn: Boolean get() = server != null && token != null

    fun signOut() = prefs.edit().remove("server").remove("email").remove("token")
        .remove("cursor").remove("last_sync").apply()
}

class ApiException(message: String) : Exception(message)

/** The few calls the app makes to a zman server. */
object Api {
    /** Tidies a server address and refuses to send a password over plain http. */
    fun normalizeServer(input: String): String {
        val trimmed = input.trim().trimEnd('/')
        val server = if (trimmed.contains("://")) trimmed else "https://$trimmed"
        val url = runCatching { URL(server) }.getOrNull()
        val host = url?.host.orEmpty()
        val local = host == "localhost" || host == "127.0.0.1" || host == "10.0.2.2"
        if (host.isEmpty() || !(url?.protocol == "https" || (url?.protocol == "http" && local))) {
            throw ApiException("The server address must start with https://")
        }
        return server
    }

    private fun post(url: String, token: String?, body: JSONObject): JSONObject {
        val connection = URL(url).openConnection() as HttpURLConnection
        try {
            connection.requestMethod = "POST"
            connection.connectTimeout = 15_000
            connection.readTimeout = 30_000
            connection.doOutput = true
            connection.setRequestProperty("Content-Type", "application/json")
            if (token != null) connection.setRequestProperty("Authorization", "Bearer $token")
            connection.outputStream.use { it.write(body.toString().toByteArray()) }

            val ok = connection.responseCode in 200..299
            val text = (if (ok) connection.inputStream else connection.errorStream)
                ?.bufferedReader()?.use { it.readText() }.orEmpty()
            val json = runCatching { JSONObject(text) }.getOrDefault(JSONObject())
            if (!ok) throw ApiException(json.optString("error", "HTTP ${connection.responseCode}"))
            return json
        } finally {
            connection.disconnect()
        }
    }

    /** Signs the device in and stores the token. The password is not kept. */
    fun login(prefs: Prefs, serverInput: String, email: String, password: String, deviceName: String) {
        val server = normalizeServer(serverInput)
        val reply = post(
            "$server/api/login", null,
            JSONObject().put("email", email).put("password", password)
                .put("device_id", prefs.deviceId).put("device_name", deviceName),
        )
        prefs.server = server
        prefs.email = reply.optString("email", email)
        prefs.token = reply.getString("token")
        prefs.cursor = 0
    }

    fun upload(prefs: Prefs, events: List<JSONObject>) {
        for (batch in events.chunked(500)) {
            post("${prefs.server}/api/events", prefs.token, JSONObject().put("events", JSONArray(batch)))
        }
    }
}

/** Reads app usage from Android and uploads it. */
object Usage {
    private const val DAY_MS = 24 * 60 * 60 * 1000L

    /** Whether the user has granted "usage access" to zman in Settings. */
    fun hasAccess(context: Context): Boolean {
        val appOps = context.getSystemService(Context.APP_OPS_SERVICE) as AppOpsManager
        @Suppress("DEPRECATION")
        val mode = appOps.checkOpNoThrow(AppOpsManager.OPSTR_GET_USAGE_STATS, Process.myUid(), context.packageName)
        return mode == AppOpsManager.MODE_ALLOWED
    }

    /** The home screen's package: time on it is not app usage. */
    private fun launcher(context: Context): Set<String> {
        val home = Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_HOME)
        val pkg = context.packageManager.resolveActivity(home, PackageManager.MATCH_DEFAULT_ONLY)?.activityInfo?.packageName
        return setOfNotNull(pkg)
    }

    fun sessions(context: Context, from: Long, to: Long): List<Session> {
        val manager = context.getSystemService(Context.USAGE_STATS_SERVICE) as UsageStatsManager
        val events = manager.queryEvents(from, to)
        val raw = mutableListOf<RawEvent>()
        val event = UsageEvents.Event()
        while (events.hasNextEvent()) {
            events.getNextEvent(event)
            raw += RawEvent(event.packageName.orEmpty(), event.eventType, event.timeStamp)
        }
        return Sessions.build(raw, to, launcher(context))
    }

    /** The name the user knows the app by ("WhatsApp"), falling back to the package. */
    private fun label(context: Context, pkg: String): String = runCatching {
        val info = context.packageManager.getApplicationInfo(pkg, 0)
        context.packageManager.getApplicationLabel(info).toString()
    }.getOrDefault(pkg)

    /**
     * Uploads recent usage. Returns how many sessions were sent.
     *
     * Each run re-reads the last half day as well as what is new: sessions
     * have stable ids, so the server just refreshes the ones that grew.
     */
    fun sync(context: Context): Int {
        val prefs = Prefs(context)
        if (!prefs.signedIn || !hasAccess(context)) return 0
        val now = System.currentTimeMillis()
        val from = if (prefs.cursor == 0L) now - 7 * DAY_MS else prefs.cursor - DAY_MS / 2
        val deviceId = prefs.deviceId
        val events = sessions(context, from, now).map { session ->
            JSONObject()
                .put("id", Sessions.id(deviceId, session))
                .put("device_id", deviceId)
                .put("start_ms", session.start)
                .put("duration_ms", session.durationMs)
                .put("app", label(context, session.pkg))
                .put("title", "")
                .put("data", JSONObject().put("package", session.pkg).put("platform", "android"))
                .put("updated_at_ms", session.end)
        }
        Api.upload(prefs, events)
        prefs.cursor = now
        prefs.lastSync = now
        return events.size
    }
}
