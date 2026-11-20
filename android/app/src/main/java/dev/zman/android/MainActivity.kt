package dev.zman.android

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.Bundle
import android.provider.Settings
import android.text.format.DateUtils
import android.view.View
import android.widget.Button
import android.widget.EditText
import android.widget.TextView
import androidx.work.Constraints
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.Worker
import androidx.work.WorkerParameters
import java.util.concurrent.TimeUnit
import kotlin.concurrent.thread

/** Uploads usage in the background every 30 minutes while there is network. */
class SyncWorker(context: Context, params: WorkerParameters) : Worker(context, params) {
    override fun doWork(): Result = try {
        Usage.sync(applicationContext)
        Result.success()
    } catch (e: Exception) {
        Result.retry()
    }

    companion object {
        fun schedule(context: Context) {
            val request = PeriodicWorkRequestBuilder<SyncWorker>(30, TimeUnit.MINUTES)
                .setConstraints(Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build())
                .build()
            WorkManager.getInstance(context)
                .enqueueUniquePeriodicWork("zman-sync", ExistingPeriodicWorkPolicy.KEEP, request)
        }
    }
}

/** The app's single screen: grant usage access, sign in, see today's top apps. */
class MainActivity : Activity() {
    private lateinit var prefs: Prefs

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)
        prefs = Prefs(this)

        findViewById<Button>(R.id.grant).setOnClickListener {
            startActivity(Intent(Settings.ACTION_USAGE_ACCESS_SETTINGS))
        }
        findViewById<Button>(R.id.sign_in).setOnClickListener { signIn() }
        findViewById<Button>(R.id.sync_now).setOnClickListener { syncNow() }
        findViewById<Button>(R.id.sign_out).setOnClickListener {
            prefs.signOut()
            WorkManager.getInstance(this).cancelUniqueWork("zman-sync")
            refresh()
        }
        if (prefs.signedIn) SyncWorker.schedule(this)
    }

    override fun onResume() {
        super.onResume()
        refresh()
    }

    private fun text(id: Int) = findViewById<TextView>(id)
    private fun show(id: Int, visible: Boolean) {
        findViewById<View>(id).visibility = if (visible) View.VISIBLE else View.GONE
    }

    private fun refresh(message: String? = null) {
        val access = Usage.hasAccess(this)
        show(R.id.access_card, !access)
        show(R.id.login_card, !prefs.signedIn)
        show(R.id.account_card, prefs.signedIn)
        show(R.id.today_card, access)

        if (prefs.signedIn) {
            val last = if (prefs.lastSync == 0L) getString(R.string.never)
            else DateUtils.getRelativeTimeSpanString(prefs.lastSync).toString()
            text(R.id.account).text = getString(R.string.signed_in_as, prefs.email, prefs.server, last)
        }
        text(R.id.message).text = message.orEmpty()
        show(R.id.message, !message.isNullOrEmpty())
        if (access) showToday()
    }

    /** Today's time per app, computed on the phone from Android's own records. */
    private fun showToday() {
        thread {
            val now = System.currentTimeMillis()
            val midnight = java.util.Calendar.getInstance().apply {
                set(java.util.Calendar.HOUR_OF_DAY, 0); set(java.util.Calendar.MINUTE, 0)
                set(java.util.Calendar.SECOND, 0); set(java.util.Calendar.MILLISECOND, 0)
            }.timeInMillis
            val perApp = Usage.sessions(this, midnight, now)
                .groupBy { it.pkg }
                .mapValues { (_, sessions) -> sessions.sumOf { it.durationMs } }
                .entries.sortedByDescending { it.value }
            val total = perApp.sumOf { it.value }
            val lines = perApp.take(8).joinToString("\n") { (pkg, ms) ->
                val label = runCatching {
                    packageManager.getApplicationLabel(packageManager.getApplicationInfo(pkg, 0)).toString()
                }.getOrDefault(pkg)
                "${duration(ms).padEnd(8)} $label"
            }
            runOnUiThread {
                text(R.id.today_total).text = duration(total)
                text(R.id.today_apps).text = lines.ifEmpty { getString(R.string.nothing_yet) }
            }
        }
    }

    private fun duration(ms: Long): String {
        val minutes = ms / 60_000
        return if (minutes >= 60) "${minutes / 60}h ${"%02d".format(minutes % 60)}m" else "${minutes}m"
    }

    private fun signIn() {
        val server = findViewById<EditText>(R.id.server).text.toString()
        val email = findViewById<EditText>(R.id.email).text.toString().trim()
        val password = findViewById<EditText>(R.id.password).text.toString()
        val button = findViewById<Button>(R.id.sign_in)
        button.isEnabled = false
        thread {
            val error = try {
                Api.login(prefs, server, email, password, "${Build.MANUFACTURER} ${Build.MODEL}")
                Usage.sync(this)
                null
            } catch (e: Exception) {
                e.message ?: e.toString()
            }
            runOnUiThread {
                button.isEnabled = true
                if (error == null) {
                    findViewById<EditText>(R.id.password).text.clear()
                    SyncWorker.schedule(this)
                }
                refresh(error)
            }
        }
    }

    private fun syncNow() {
        val button = findViewById<Button>(R.id.sync_now)
        button.isEnabled = false
        thread {
            val message = try {
                getString(R.string.synced, Usage.sync(this))
            } catch (e: Exception) {
                e.message ?: e.toString()
            }
            runOnUiThread {
                button.isEnabled = true
                refresh(message)
            }
        }
    }
}
