package jp.nonbili.meron.ui

import jp.nonbili.meron.shared.FolderSummary

/**
 * A folder in the hierarchy built from IMAP path names. [folder] is null for a
 * structural node — a path segment that only exists as a parent of real folders.
 */
internal data class FolderTreeNode(
    val name: String,
    val folder: FolderSummary?,
    val children: List<FolderTreeNode>,
)

/** A tree node paired with its depth, for rendering the tree as a flat list. */
internal data class FolderTreeRow(
    val node: FolderTreeNode,
    val depth: Int,
)

/** Pick the hierarchy delimiter: prefer the server-reported one, else infer. */
internal fun folderTreeDelimiter(folders: List<FolderSummary>): String =
    when {
        folders.any { it.delimiter.isNotEmpty() } -> folders.first { it.delimiter.isNotEmpty() }.delimiter
        folders.any { it.name.contains('/') } -> "/"
        folders.any { it.name.contains('.') } -> "."
        else -> "/"
    }

/**
 * The folder a "delete folder" action may target: an ordinary folder of a single
 * account. Special-use folders carry the app's own routing
 * (Inbox/Sent/Drafts/Trash/Junk/Archive), and that rules out a parent holding one
 * as well, because deleting a folder takes everything nested under it too. The
 * core re-checks both rules.
 */
internal fun deletableFolder(
    folders: List<FolderSummary>,
    accountId: String,
    folderId: String,
): FolderSummary? {
    if (accountId == UNIFIED_ACCOUNT_ID) return null
    val folder = folders.firstOrNull { it.accountId == accountId && it.name == folderId } ?: return null
    if (folder.role != "folder") return null
    if (nestedFolders(folders, accountId, folderId).any { it.role != "folder" }) return null
    return folder
}

/** The folder [folderId] names when it can be opted in to background sync and
 *  new-mail notifications: a real mailbox of one account that is neither the
 *  inbox (always watched) nor a place mail does not arrive in. */
internal fun notifiableFolder(
    folders: List<FolderSummary>,
    accountId: String,
    folderId: String,
): FolderSummary? {
    if (accountId == UNIFIED_ACCOUNT_ID) return null
    val folder = folders.firstOrNull { it.accountId == accountId && it.name == folderId } ?: return null
    return folder.takeUnless { it.role in setOf("inbox", "drafts", "sent", "trash", "junk") }
}

/** The folders nested under [folderId], which a delete of it would take along. */
internal fun nestedFolders(
    folders: List<FolderSummary>,
    accountId: String,
    folderId: String,
): List<FolderSummary> {
    val prefix = folderId + folderTreeDelimiter(folders)
    return folders.filter { it.accountId == accountId && it.name != folderId && it.name.startsWith(prefix) }
}

private class MutableFolderTreeNode(
    val name: String,
    var folder: FolderSummary? = null,
    val children: MutableList<MutableFolderTreeNode> = mutableListOf(),
) {
    fun toNode(): FolderTreeNode = FolderTreeNode(name, folder, children.map { it.toNode() })
}

/** Group folders into a tree by splitting their names on the hierarchy delimiter. */
internal fun buildFolderTree(folders: List<FolderSummary>): List<FolderTreeNode> {
    val delimiter = folderTreeDelimiter(folders)
    val roots = mutableListOf<MutableFolderTreeNode>()
    val byPath = mutableMapOf<String, MutableFolderTreeNode>()
    folders.forEach { folder ->
        val segments =
            folder.name
                .split(delimiter)
                .filter { it.isNotEmpty() }
                .ifEmpty { listOf(folder.name) }
        // Paths stay on the wire name (folder identity); labels come from the
        // decoded one. A decoded segment can itself contain the delimiter, so
        // only use the decoded split when it lines up segment for segment.
        val displaySegments =
            folder.displayName
                .split(delimiter)
                .filter { it.isNotEmpty() }
                .takeIf { it.size == segments.size }
        var siblings = roots
        var path = ""
        segments.forEachIndexed { index, segment ->
            path = if (path.isEmpty()) segment else "$path$delimiter$segment"
            val node =
                byPath.getOrPut(path) {
                    MutableFolderTreeNode(displaySegments?.get(index) ?: segment).also { siblings.add(it) }
                }
            // The final segment is the real folder; intermediates may be structural.
            if (index == segments.lastIndex) node.folder = folder
            siblings = node.children
        }
    }
    return roots.map { it.toNode() }
}

/** Depth-first flattening of [nodes], so the tree can be laid out as menu rows. */
internal fun flattenFolderTree(
    nodes: List<FolderTreeNode>,
    depth: Int = 0,
): List<FolderTreeRow> =
    nodes.flatMap { node ->
        listOf(FolderTreeRow(node, depth)) + flattenFolderTree(node.children, depth + 1)
    }
