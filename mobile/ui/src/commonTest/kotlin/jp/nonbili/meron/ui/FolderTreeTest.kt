package jp.nonbili.meron.ui

import jp.nonbili.meron.shared.FolderSummary
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

class FolderTreeTest {
    private fun folder(name: String) = FolderSummary(accountId = "acc1", name = name)

    @Test
    fun nestsFoldersUnderTheirParentPath() {
        val tree = buildFolderTree(listOf(folder("INBOX"), folder("Work/Clients"), folder("Work/Clients/Acme"), folder("Work")))

        assertEquals(listOf("INBOX", "Work"), tree.map { it.name })
        val work = tree.last()
        assertEquals(listOf("Clients"), work.children.map { it.name })
        assertEquals(
            listOf("Acme"),
            work.children
                .first()
                .children
                .map { it.name },
        )
        assertEquals(
            "Work/Clients/Acme",
            work.children
                .first()
                .children
                .first()
                .folder
                ?.name,
        )
    }

    @Test
    fun prefersTheServerReportedDelimiterOverInference() {
        val folders =
            listOf(
                FolderSummary(accountId = "acc1", name = "INBOX", delimiter = "."),
                FolderSummary(accountId = "acc1", name = "INBOX.Work", delimiter = "."),
            )

        assertEquals(".", folderTreeDelimiter(folders))
        val inbox = buildFolderTree(folders).single()
        assertEquals(listOf("Work"), inbox.children.map { it.name })
    }

    @Test
    fun infersTheDelimiterWhenTheServerReportsNone() {
        assertEquals("/", folderTreeDelimiter(listOf(folder("Work/Clients"))))
        assertEquals(".", folderTreeDelimiter(listOf(folder("INBOX.Work"))))
        assertEquals("/", folderTreeDelimiter(listOf(folder("INBOX"))))
    }

    @Test
    fun labelsNodesWithTheDecodedNameAndKeepsWirePathsForIdentity() {
        val encoded = FolderSummary(accountId = "acc1", name = "Work/gds-&AOQA5A--envoy&AOk-s", displayName = "Work/gds-ää-envoyés")

        val work = buildFolderTree(listOf(folder("Work"), encoded)).single()

        assertEquals(listOf("gds-ää-envoyés"), work.children.map { it.name })
        assertEquals(
            "Work/gds-&AOQA5A--envoy&AOk-s",
            work.children
                .single()
                .folder
                ?.name,
        )
    }

    @Test
    fun keepsMissingIntermediatesAsStructuralNodes() {
        val tree = buildFolderTree(listOf(folder("Work/Clients/Acme")))

        val work = tree.single()
        assertNull(work.folder)
        assertEquals(
            "Acme",
            work.children
                .single()
                .children
                .single()
                .name,
        )
    }

    @Test
    fun infersDotDelimiterWhenNoSlashIsUsed() {
        val tree = buildFolderTree(listOf(folder("INBOX.Sent"), folder("INBOX.Drafts")))

        assertEquals(listOf("INBOX"), tree.map { it.name })
        assertEquals(listOf("Sent", "Drafts"), tree.single().children.map { it.name })
    }

    @Test
    fun offersDeleteForAParentAndCountsWhatGoesWithIt() {
        val folders = listOf(folder("INBOX"), folder("Work"), folder("Work/Acme"), folder("Work/Acme/2026"), folder("Workshop"))

        // Nesting is no bar: the delete takes the subtree along. The lookalike
        // sibling that merely shares a name prefix stays out of it.
        assertEquals("Work", deletableFolder(folders, "acc1", "Work")?.name)
        assertEquals(listOf("Work/Acme", "Work/Acme/2026"), nestedFolders(folders, "acc1", "Work").map { it.name })
        assertEquals(0, nestedFolders(folders, "acc1", "Workshop").size)
    }

    @Test
    fun refusesDeleteWhenASpecialUseFolderIsInTheSubtree() {
        val folders =
            listOf(
                FolderSummary(accountId = "acc1", name = "INBOX", role = "inbox"),
                FolderSummary(accountId = "acc1", name = "Mail"),
                FolderSummary(accountId = "acc1", name = "Mail/Archive", role = "archive"),
            )

        assertNull(deletableFolder(folders, "acc1", "INBOX"))
        assertNull(deletableFolder(folders, "acc1", "Mail"))
        assertNull(deletableFolder(folders, UNIFIED_ACCOUNT_ID, "Mail"))
    }

    @Test
    fun flattensDepthFirstWithDepths() {
        val rows = flattenFolderTree(buildFolderTree(listOf(folder("Work"), folder("Work/Acme"), folder("Personal"))))

        assertEquals(listOf("Work" to 0, "Acme" to 1, "Personal" to 0), rows.map { it.node.name to it.depth })
        assertTrue(rows.all { it.node.folder != null })
    }

    @Test
    fun threadFolderLabelTranslatesInboxAndShowsTheDecodedLeaf() {
        val folders =
            listOf(
                FolderSummary(accountId = "acc1", name = "INBOX", role = "inbox"),
                FolderSummary(accountId = "acc1", name = "[Gmail]/Sent Mail", role = "sent", delimiter = "/"),
                FolderSummary(accountId = "acc1", name = "t2/&MMYwuTDI-", displayName = "t2/テスト", delimiter = "/"),
                FolderSummary(accountId = "acc1", name = "a.b"),
            )

        assertEquals("Inbox", threadFolderLabel("INBOX", folders, "Inbox"))
        assertEquals("Inbox", threadFolderLabel("INBOX", emptyList(), "Inbox"))
        assertEquals("Sent Mail", threadFolderLabel("[Gmail]/Sent Mail", folders, "Inbox"))
        assertEquals("テスト", threadFolderLabel("t2/&MMYwuTDI-", folders, "Inbox"))
        // No delimiter: punctuation is part of the name.
        assertEquals("a.b", threadFolderLabel("a.b", folders, "Inbox"))
        // Folders not loaded yet: the bare id rather than nothing.
        assertEquals("Archive", threadFolderLabel("Archive", folders, "Inbox"))
    }
}
