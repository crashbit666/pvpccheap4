package com.crashbit.pvpccheap4.alerts

import android.Manifest
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import com.crashbit.pvpccheap4.MainActivity
import com.crashbit.pvpccheap4.R
import com.crashbit.pvpccheap4.data.local.TokenManager
import com.crashbit.pvpccheap4.data.repository.DeviceRepository
import com.crashbit.pvpccheap4.data.repository.Result as DataResult
import dagger.hilt.EntryPoint
import dagger.hilt.InstallIn
import dagger.hilt.android.EntryPointAccessors
import dagger.hilt.components.SingletonComponent
import java.util.concurrent.TimeUnit

/**
 * Every 15 minutes asks the server for device actions that failed after the
 * retries (e.g. a plug that could not be turned off) and shows each new one
 * as a notification.
 */
class AlertsWorker(
    context: Context,
    params: WorkerParameters
) : CoroutineWorker(context, params) {

    @EntryPoint
    @InstallIn(SingletonComponent::class)
    interface Dependencies {
        fun deviceRepository(): DeviceRepository
        fun tokenManager(): TokenManager
    }

    override suspend fun doWork(): Result {
        val deps = EntryPointAccessors.fromApplication(applicationContext, Dependencies::class.java)
        val tokenManager = deps.tokenManager()
        if (tokenManager.getTokenSync() == null) return Result.success()

        val alerts = when (val result = deps.deviceRepository().getAlerts()) {
            is DataResult.Success -> result.data
            else -> return Result.retry()
        }
        val lastShown = tokenManager.lastNotifiedAlert()
        val fresh = alerts.filter { it.executionId > lastShown }
        if (fresh.isEmpty()) return Result.success()

        if (canNotify(applicationContext)) {
            ensureChannel(applicationContext)
            fresh.forEach { alert ->
                notify(applicationContext, alert.executionId, alert.title, alert.detail)
            }
        }
        tokenManager.setLastNotifiedAlert(fresh.maxOf { it.executionId })
        return Result.success()
    }

    companion object {
        private const val WORK_NAME = "device-alerts"
        private const val CHANNEL_ID = "device_alerts"

        fun schedule(context: Context) {
            val request = PeriodicWorkRequestBuilder<AlertsWorker>(15, TimeUnit.MINUTES)
                .setConstraints(
                    Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build()
                )
                .build()
            WorkManager.getInstance(context)
                .enqueueUniquePeriodicWork(WORK_NAME, ExistingPeriodicWorkPolicy.KEEP, request)
        }

        fun cancel(context: Context) {
            WorkManager.getInstance(context).cancelUniqueWork(WORK_NAME)
        }

        fun canNotify(context: Context): Boolean =
            ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) ==
                PackageManager.PERMISSION_GRANTED

        private fun ensureChannel(context: Context) {
            val channel = NotificationChannel(
                CHANNEL_ID,
                "Avisos dels dispositius",
                NotificationManager.IMPORTANCE_HIGH
            ).apply {
                description = "Quan un endoll no s'ha pogut encendre o apagar"
            }
            context.getSystemService(NotificationManager::class.java).createNotificationChannel(channel)
        }

        @Suppress("MissingPermission") // checked by canNotify()
        private fun notify(context: Context, id: Int, title: String, text: String) {
            val open = PendingIntent.getActivity(
                context,
                0,
                Intent(context, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
                PendingIntent.FLAG_IMMUTABLE
            )
            val notification = NotificationCompat.Builder(context, CHANNEL_ID)
                .setSmallIcon(R.drawable.ic_warning)
                .setContentTitle(title)
                .setContentText(text)
                .setContentIntent(open)
                .setAutoCancel(true)
                .setPriority(NotificationCompat.PRIORITY_HIGH)
                .build()
            NotificationManagerCompat.from(context).notify(id, notification)
        }
    }
}
