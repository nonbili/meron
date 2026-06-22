package jp.nonbili.meron

import android.app.NotificationManager
import android.os.Build
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Test

class AndroidNotificationContractTest {
    @Test
    fun refreshNotificationChannelIsCreated() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return

        val context = InstrumentationRegistry.getInstrumentation().targetContext
        AndroidNotificationService.ensureChannels(context)

        val manager = context.getSystemService(NotificationManager::class.java)
        val channel = manager.getNotificationChannel(AndroidNotificationService.refreshChannelIdForTesting())
        assertNotNull(channel)
        assertEquals("Mail sync", channel.name.toString())
        assertEquals(NotificationManager.IMPORTANCE_DEFAULT, channel.importance)
    }
}
