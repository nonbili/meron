package jp.nonbili.meron.shared

import kotlin.test.Test
import kotlin.test.assertEquals

class ThreadGalleryMediaTest {
    @Test
    fun galleryImagesKeepThreadOrderAndMediaRefs() {
        val messages =
            listOf(
                message("m1", image("one.png", key = "acct/one.png")),
                message("m2", image("two.png", url = "https://example.com/two.png")),
            )

        val images = buildThreadGalleryImages(messages, allowRemote = { true })

        assertEquals(listOf("one.png", "two.png"), images.map { it.filename })
        assertEquals(listOf("/media/acct/one.png", "https://example.com/two.png"), images.map { it.ref })
        assertEquals(listOf("m1", "m2"), images.map { it.messageId })
    }

    @Test
    fun mediaItemsAreNewestFirstAndKeepImageGalleryIndexes() {
        val messages =
            listOf(
                message(
                    "m1",
                    image("one.png", key = "acct/one.png"),
                    file("one.pdf"),
                ),
                message(
                    "m2",
                    video("clip.mp4", key = "acct/clip.mp4"),
                    image("two.jpg", key = "acct/two.jpg"),
                ),
            )

        val media = buildThreadMediaItems(messages, allowRemote = { true })

        assertEquals(listOf("two.jpg", "clip.mp4", "one.png"), media.map { it.filename })
        assertEquals(listOf("image", "video", "image"), media.map { it.type })
        assertEquals(listOf(1, null, 0), media.map { it.galleryIndex })
    }

    @Test
    fun attachmentsWithoutResolvableMediaRefsAreDropped() {
        val messages =
            listOf(
                message(
                    "m1",
                    image("cached.png", key = "acct/cached.png"),
                    image("missing.png"),
                    video("remote.mp4", url = "https://example.com/remote.mp4"),
                ),
            )

        assertEquals(listOf("cached.png"), buildThreadGalleryImages(messages, allowRemote = { true }).map { it.filename })
        assertEquals(listOf("remote.mp4", "cached.png"), buildThreadMediaItems(messages, allowRemote = { true }).map { it.filename })
    }

    @Test
    fun blockedRemoteMediaIsLeftOutOfTheGalleryAndPanel() {
        val messages =
            listOf(
                message("m1", image("cached.png", key = "acct/cached.png")),
                message("m2", image("tracker.png", url = "https://example.com/tracker.png")),
                message("m3", video("clip.mp4", url = "https://example.com/clip.mp4")),
            )
        val allowRemote = { message: MessageBody -> message.id == "m3" }

        // The gallery holds the cached image and the revealed message's media
        // only, and the panel indexes into exactly that list.
        assertEquals(listOf("cached.png"), buildThreadGalleryImages(messages, allowRemote).map { it.filename })
        val media = buildThreadMediaItems(messages, allowRemote)
        assertEquals(listOf("clip.mp4", "cached.png"), media.map { it.filename })
        assertEquals(listOf(null, 0), media.map { it.galleryIndex })
    }

    private fun message(
        id: String,
        vararg attachments: MessageAttachment,
    ): MessageBody =
        MessageBody(
            id = id,
            from = "Sender",
            to = "me@example.com",
            subject = "Subject",
            body = "Body",
            attachments = attachments.toList(),
        )

    private fun image(
        filename: String,
        key: String = "",
        url: String = "",
    ): MessageAttachment = MessageAttachment(filename = filename, mimeType = "image/png", key = key, url = url)

    private fun video(
        filename: String,
        key: String = "",
        url: String = "",
    ): MessageAttachment = MessageAttachment(filename = filename, mimeType = "video/mp4", key = key, url = url)

    private fun file(filename: String): MessageAttachment = MessageAttachment(filename = filename, mimeType = "application/pdf", key = "acct/$filename")
}
