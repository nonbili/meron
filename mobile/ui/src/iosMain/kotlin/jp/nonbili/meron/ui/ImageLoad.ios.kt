package jp.nonbili.meron.ui

import kotlinx.cinterop.ExperimentalForeignApi
import kotlinx.cinterop.addressOf
import kotlinx.cinterop.usePinned
import platform.Foundation.NSApplicationSupportDirectory
import platform.Foundation.NSData
import platform.Foundation.NSFileManager
import platform.Foundation.NSURL
import platform.Foundation.NSUserDomainMask
import platform.Foundation.dataWithContentsOfFile
import platform.Foundation.dataWithContentsOfURL
import platform.posix.memcpy

@OptIn(ExperimentalForeignApi::class)
actual suspend fun loadImageBytes(ref: String): ByteArray? {
    val data: NSData? =
        when {
            ref.startsWith("/media/") -> {
                val relative = ref.removePrefix("/media/").trimStart('/')
                if (relative.isBlank() || relative.split('/').any { it == ".." || it.isBlank() }) return null
                val base =
                    NSFileManager.defaultManager
                        .URLsForDirectory(NSApplicationSupportDirectory, NSUserDomainMask)
                        .firstOrNull() as? NSURL ?: return null
                val url = base.URLByAppendingPathComponent("media")?.URLByAppendingPathComponent(relative)
                url?.let { NSData.dataWithContentsOfURL(it) }
            }

            ref.startsWith("http://") || ref.startsWith("https://") -> {
                val url = NSURL.URLWithString(ref) ?: return null
                NSData.dataWithContentsOfURL(url)
            }

            else -> NSData.dataWithContentsOfFile(ref)
        }
    return data?.toByteArray()
}

@OptIn(ExperimentalForeignApi::class)
private fun NSData.toByteArray(): ByteArray {
    val size = length.toInt()
    if (size == 0) return ByteArray(0)
    val out = ByteArray(size)
    out.usePinned { pinned ->
        memcpy(pinned.addressOf(0), bytes, length)
    }
    return out
}
