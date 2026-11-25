package jp.nonbili.meron.ui

import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.SnackbarHost
import androidx.compose.runtime.Composable
import androidx.compose.runtime.key
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.backhandler.BackHandler
import androidx.compose.ui.input.key.key
import jp.nonbili.meron.shared.DraftAttachment
import kotlin.io.encoding.ExperimentalEncodingApi

@OptIn(ExperimentalMaterial3Api::class, ExperimentalEncodingApi::class, ExperimentalComposeUiApi::class)
@Composable
internal fun ComposeRouteContent(
    state: MeronMobileState,
    pickAttachmentInto: ((DraftAttachment) -> Unit) -> Unit,
) {
    with(state) {
        BackHandler(
            onBack = { closeCompose() },
        )
        ComposeScreen(
            sendIdentities = composeIdentityCandidates(),
            selectedFromKey = selectedComposeIdentity()?.let { identityKey(it) }.orEmpty(),
            onFromChange = { key ->
                val split = key.indexOf('|')
                if (split > 0) {
                    changeComposeIdentity(key.substring(0, split), key.substring(split + 1))
                }
            },
            to = to,
            onToChange = {
                to = it
                loadRecipientSuggestions("to", it)
            },
            cc = cc,
            onCcChange = {
                cc = it
                loadRecipientSuggestions("cc", it)
            },
            bcc = bcc,
            onBccChange = {
                bcc = it
                loadRecipientSuggestions("bcc", it)
            },
            subject = subject,
            onSubjectChange = { subject = it },
            body = body,
            onBodyChange = { body = it },
            hasFoldedQuote = composeFoldedQuote != null,
            onExpandQuote = {
                expandComposeQuote()
                autoSaveComposeDraft()
            },
            onRemoveQuote = {
                removeComposeQuote()
                autoSaveComposeDraft()
            },
            attachments = attachments,
            recipientSuggestionField = recipientSuggestionField,
            recipientSuggestions = recipientSuggestions,
            onRecipientFocus = { field, value -> loadRecipientSuggestions(field, value) },
            onAcceptRecipientSuggestion = ::acceptRecipientSuggestion,
            onAttach = {
                pickAttachmentInto { picked ->
                    attachments = attachments + picked
                }
            },
            onClearAttachments = { attachments = emptyList() },
            onRemoveAttachment = { draftAttachment ->
                attachments = attachments.filter { it.id != draftAttachment.id }
            },
            sendShortcutMode = sendShortcutMode,
            onSaveDraft = ::saveComposeDraft,
            onAutoSaveDraft = ::autoSaveComposeDraft,
            onDiscardDraft = ::discardComposeDraft,
            onSend = ::sendMail,
            onBack = ::closeCompose,
            sending = composeSendInFlight,
            snackbarHost = { SnackbarHost(snackbarHost) },
        )
    }
}
