package jp.nonbili.meron.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Contacts
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.outlined.Email
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.compositeOver
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import jp.nonbili.meron.shared.AccountSummary
import jp.nonbili.meron.shared.ContactSuggestion
import jp.nonbili.meron.shared.FolderSummary
import jp.nonbili.meron.shared.ThreadSummary
import jp.nonbili.meron.shared.formatContactSuggestion
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun ContactDetails(
    state: ContactsScreenState,
    contact: ContactSuggestion,
    onCompose: (String) -> Unit,
    onOpenThread: (ThreadSummary) -> Unit,
    onCopy: (String, String) -> Unit,
    canCompose: Boolean,
    accounts: List<AccountSummary>,
    foldersByAccount: Map<String, List<FolderSummary>>,
    showSenderImages: Boolean,
    onEditCard: (BookCard) -> Unit = {},
    onSaveToBook: (note: String) -> Unit = {},
    onMoveNote: (BookCard, String) -> Unit = { _, _ -> },
) {
    val scope = rememberCoroutineScope()
    val colors = MaterialTheme.colorScheme
    val copyLabel = tr("chat.copyEmailAddress")
    var copied by remember(contact.addr) { mutableStateOf(false) }
    val listState = rememberLazyListState()
    val accountsById = remember(accounts) { accounts.associateBy { it.id } }
    val ownAccount =
        remember(accounts, contact.addr) {
            val addr = contact.addr.trim()
            accounts.firstOrNull { account ->
                account.email.trim().equals(addr, ignoreCase = true) ||
                    account.aliases.any { it.email.trim().equals(addr, ignoreCase = true) }
            }
        }
    val inboxLabel = tr("folders.roles.inbox")
    val displayName = contact.name.ifBlank { contact.addr }
    val hasDistinctName = contact.name.isNotBlank() && !contact.name.equals(contact.addr, ignoreCase = true)
    LaunchedEffect(copied) {
        if (copied) {
            delay(1500)
            copied = false
        }
    }
    LazyColumn(
        Modifier.fillMaxSize().appScrollbar(listState),
        state = listState,
        contentPadding = PaddingValues(bottom = 24.dp + navigationBarInset()),
    ) {
        item(key = "header:${contact.addr}") {
            Row(
                Modifier
                    .fillMaxWidth()
                    .padding(start = 16.dp, end = 16.dp, top = 16.dp, bottom = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(14.dp),
            ) {
                // Our own address is drawn as the account, not as whatever the
                // address resolves to (the favicon of our own domain, say).
                if (ownAccount != null) {
                    AccountBadgeAvatar(
                        label = ownAccount.displayName.ifBlank { displayName },
                        avatarUrl = ownAccount.avatarUrl,
                        size = 48.dp,
                        fallbackIcon = accountAvatarFallbackIcon(ownAccount),
                    )
                } else {
                    SenderAvatar(
                        formatContactSuggestion(contact),
                        enabled = showSenderImages,
                        size = 48.dp,
                        fallbackName = displayName,
                    )
                }
                Column(
                    Modifier.weight(1f),
                    verticalArrangement = Arrangement.spacedBy(2.dp),
                ) {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(4.dp),
                    ) {
                        Text(
                            displayName,
                            style = MaterialTheme.typography.titleMedium,
                            fontWeight = FontWeight.SemiBold,
                            color = colors.onSurface,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.weight(1f, fill = false),
                        )
                        if (!hasDistinctName) {
                            ContactCopyAddressButton(
                                copied = copied,
                                onClick = {
                                    onCopy(copyLabel, contact.addr)
                                    copied = true
                                },
                            )
                        }
                    }
                    if (hasDistinctName) {
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(4.dp),
                        ) {
                            SelectionContainer(Modifier.weight(1f, fill = false)) {
                                Text(
                                    contact.addr,
                                    style = MaterialTheme.typography.bodySmall,
                                    color = colors.onSurfaceVariant,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                            }
                            ContactCopyAddressButton(
                                copied = copied,
                                onClick = {
                                    onCopy(copyLabel, contact.addr)
                                    copied = true
                                },
                            )
                        }
                    }
                }
                if (canCompose) {
                    FilledTonalIconButton(
                        onClick = { onCompose(formatContactSuggestion(contact)) },
                        colors =
                            IconButtonDefaults.filledTonalIconButtonColors(
                                containerColor = colors.primaryContainer,
                                contentColor = colors.onPrimaryContainer,
                            ),
                        modifier = Modifier.size(40.dp),
                    ) {
                        Icon(
                            Icons.Outlined.Email,
                            contentDescription = tr("composer.actions.newMessage"),
                            modifier = Modifier.size(19.dp),
                        )
                    }
                }
            }
        }
        // Saved in address books: each card, shared with other clients.
        if (state.cardsFailed == contact.addr) {
            item(key = "cards:failed") {
                TextButton(onClick = { scope.launch { state.loadCards(contact.addr) } }, modifier = Modifier.padding(horizontal = 8.dp)) {
                    Text(tr("contacts.addressBookRetry"))
                }
            }
        }
        val cards = if (state.cardsAddr == contact.addr) state.cards else emptyList()
        items(cards, key = { "card:${it.bookId}\n${it.id}" }) { card -> BookCardSection(card, onEdit = { onEditCard(card) }) }
        item(key = "note:${contact.addr}") { ContactNotes(state, contact.addr) }
        // The note kept only on this device can move to a card, or go with a
        // new one: the user's choice, as anyone with the book will see it.
        if (state.cardsAddr == contact.addr && state.books.isNotEmpty()) {
            item(key = "note-actions:${contact.addr}") {
                val note = state.note(contact.addr)
                val localNote = if (note.loaded) note.saved.trim() else ""
                FlowRow(Modifier.padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    if (cards.isEmpty()) {
                        OutlinedButton(
                            enabled = note.loaded || note.error,
                            onClick = { onSaveToBook(localNote) },
                        ) { Text(tr("contacts.saveToAddressBook")) }
                    } else if (localNote.isNotBlank()) {
                        cards.forEach { card ->
                            OutlinedButton(onClick = { onMoveNote(card, localNote) }) {
                                Text(tr("contacts.moveNote", mapOf("book" to card.bookName)), maxLines = 1, overflow = TextOverflow.Ellipsis)
                            }
                        }
                    }
                }
            }
        }
        item(key = "section:conversations") {
            HorizontalDivider(color = colors.outlineVariant.copy(alpha = 0.5f))
            ContactSectionTitle(tr("contacts.recentConversations"))
        }
        items(state.threads, key = { it.id }) { thread ->
            ContactThreadRow(
                thread = thread,
                account = accountsById[thread.accountId],
                // History spans folders, so each row names where it lives.
                folderLabel =
                    thread.folder
                        .takeIf { it.isNotBlank() }
                        ?.let { folder ->
                            threadFolderLabel(folder, foldersByAccount[thread.accountId].orEmpty(), inboxLabel)
                        }.orEmpty(),
                showSenderImages = showSenderImages,
                onOpen = { onOpenThread(thread) },
            )
            HorizontalDivider(
                Modifier.padding(start = 66.dp, end = 16.dp),
                color = colors.outlineVariant.copy(alpha = 0.35f),
            )
        }
        if (state.historyLoading) item(key = "history:loading") { ContactLoading() }
        if (state.historyError) {
            item(key = "history:error") {
                ContactFailure { scope.launch { state.loadHistory(more = state.historyLoaded) } }
            }
        }
        if (state.historyLoaded && !state.historyLoading && !state.historyError) {
            if (state.threads.isEmpty()) {
                item(key = "history:empty") {
                    Text(
                        tr("contacts.noConversations"),
                        Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                        style = MaterialTheme.typography.bodyMedium,
                        color = colors.onSurfaceVariant,
                    )
                }
            }
            if (state.cursor.isNotEmpty()) {
                item(key = "history:more") {
                    Box(Modifier.fillMaxWidth().padding(vertical = 12.dp), contentAlignment = Alignment.Center) {
                        OutlinedButton(onClick = { scope.launch { state.loadHistory(more = true) } }) {
                            Text(tr("threads.actions.loadMore"))
                        }
                    }
                }
            }
            item(key = "history:footnote") {
                Text(
                    tr("contacts.cachedHistory"),
                    Modifier.padding(horizontal = 16.dp, vertical = 12.dp),
                    style = MaterialTheme.typography.labelSmall,
                    color = colors.onSurfaceVariant.copy(alpha = 0.75f),
                )
            }
        }
    }
}

/** One contact as an address book holds it: where, the name, the addresses
 *  and the shared note, with Edit. */
@Composable
private fun BookCardSection(
    card: BookCard,
    onEdit: () -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    Surface(
        shape = RoundedCornerShape(16.dp),
        color = colors.surfaceContainerLow,
        modifier = Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 4.dp),
    ) {
        Column(Modifier.padding(start = 16.dp, end = 4.dp, top = 8.dp, bottom = 12.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(Icons.Filled.Contacts, contentDescription = null, modifier = Modifier.size(16.dp), tint = colors.onSurfaceVariant)
                Text(
                    bookLabel(card.bookName, card.account),
                    style = MaterialTheme.typography.labelMedium,
                    color = colors.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f).padding(start = 8.dp),
                )
                IconButton(onClick = onEdit) {
                    Icon(Icons.Filled.Edit, contentDescription = tr("contacts.editContact"), modifier = Modifier.size(18.dp))
                }
            }
            if (card.name.isNotBlank()) Text(card.name, fontWeight = FontWeight.SemiBold)
            card.emails.forEach { Text(it, style = MaterialTheme.typography.bodySmall, color = colors.onSurfaceVariant) }
            if (card.note.isNotBlank()) {
                Spacer(Modifier.height(8.dp))
                ContactMarkdown(card.note)
            }
        }
    }
}

@Composable
private fun ContactCopyAddressButton(
    copied: Boolean,
    onClick: () -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    IconButton(
        onClick = onClick,
        modifier = Modifier.size(26.dp),
    ) {
        Icon(
            if (copied) Icons.Filled.Check else Icons.Filled.ContentCopy,
            contentDescription = tr(if (copied) "common.copied" else "chat.copyEmailAddress"),
            tint = if (copied) colors.primary else colors.onSurfaceVariant,
            modifier = Modifier.size(14.dp),
        )
    }
}

@Composable
private fun ContactThreadRow(
    thread: ThreadSummary,
    account: AccountSummary?,
    folderLabel: String,
    showSenderImages: Boolean,
    onOpen: () -> Unit,
) {
    val unread = thread.unread
    val colors = MaterialTheme.colorScheme
    val chat = LocalChatColors.current
    val senderLabel = thread.sender.ifBlank { thread.accountId }
    val showAvatar = LocalShowThreadAvatars.current
    val accountColor = account?.let { accountColorOrNull(it.color) }
    val badgeAccount = account.takeIf { accountColor == null }
    val hasDraft = thread.hasDraft || thread.folderRole == "drafts"
    val draftBadge = tr("chat.draft")
    val rowBackground =
        if (unread) {
            colors.primary.copy(alpha = 0.06f).compositeOver(colors.background)
        } else {
            colors.background
        }
    Row(
        Modifier
            .fillMaxWidth()
            .background(rowBackground)
            .accountColorEdge(accountColor)
            .clickable(onClick = onOpen)
            .padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        if (showAvatar) {
            ThreadAvatarWithAccountBadge(
                senderLabel = senderLabel,
                showSenderImages = showSenderImages,
                account = badgeAccount,
                senderAddr = thread.senderAddr,
            )
        }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                Row(
                    Modifier.weight(1f),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    if (!showAvatar && badgeAccount != null) InlineAccountBadge(badgeAccount)
                    val me = tr("threads.me")
                    Text(
                        if (thread.senders.size > 1) {
                            val names = thread.senders.map { it.ifBlank { me } }
                            if (thread.sendersTruncated) {
                                names.first() + " .. " + names.drop(1).joinToString(", ")
                            } else {
                                names.joinToString(", ")
                            }
                        } else {
                            senderLabel.substringBefore('@')
                        },
                        fontSize = 13.sp,
                        fontWeight = if (unread) FontWeight.SemiBold else FontWeight.Medium,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.weight(1f, fill = false),
                        color = colors.onSurface,
                    )
                    if (thread.messageCount > 1) {
                        Text(
                            thread.messageCount.toString(),
                            fontSize = 12.sp,
                            fontWeight = FontWeight.Normal,
                            color = colors.onSurfaceVariant,
                            modifier = Modifier.padding(start = 4.dp),
                        )
                    }
                }
                if (folderLabel.isNotBlank()) {
                    Surface(
                        modifier = Modifier.widthIn(max = 96.dp),
                        shape = RoundedCornerShape(4.dp),
                        color = colors.surfaceContainerHigh,
                    ) {
                        Text(
                            folderLabel,
                            modifier = Modifier.padding(horizontal = 5.dp, vertical = 1.dp),
                            fontSize = 10.sp,
                            fontWeight = FontWeight.Medium,
                            color = colors.onSurfaceVariant,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                    }
                }
                if (thread.dateEpochSeconds > 0) {
                    Text(
                        formatInboxTimestamp(thread.dateEpochSeconds),
                        fontSize = 11.sp,
                        fontWeight = if (unread) FontWeight.Medium else FontWeight.Normal,
                        color = if (unread) colors.onSurface else colors.onSurfaceVariant,
                        maxLines = 1,
                    )
                }
                if (unread) {
                    val unreadContentDescription = tr("filters.unread")
                    Box(
                        Modifier
                            .size(8.dp)
                            .clip(CircleShape)
                            .background(colors.primary)
                            .semantics { contentDescription = unreadContentDescription },
                    )
                }
            }
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(5.dp),
            ) {
                if (thread.starred) {
                    Icon(
                        Icons.Filled.Star,
                        contentDescription = null,
                        tint = chat.star,
                        modifier = Modifier.size(14.dp),
                    )
                }
                Text(
                    buildAnnotatedString {
                        if (hasDraft) {
                            withStyle(
                                SpanStyle(
                                    color = colors.error,
                                    fontWeight = FontWeight.Normal,
                                ),
                            ) {
                                append(draftBadge)
                                append(" ")
                            }
                        }
                        withStyle(
                            SpanStyle(
                                color = colors.onSurface,
                                fontWeight = if (unread) FontWeight.SemiBold else FontWeight.Medium,
                            ),
                        ) {
                            append(thread.subject.ifBlank { "(no subject)" })
                        }
                        if (thread.preview.isNotBlank()) {
                            withStyle(SpanStyle(color = colors.onSurfaceVariant, fontWeight = FontWeight.Normal)) {
                                append(" — ${thread.preview}")
                            }
                        }
                    },
                    fontSize = 12.sp,
                    lineHeight = 17.sp,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f),
                )
            }
            MailRowTagChips(thread.tags, Modifier.padding(top = 4.dp))
            if (thread.files.isNotEmpty()) {
                MailRowAttachmentChips(thread.files, Modifier.padding(top = 4.dp))
            }
        }
    }
}
