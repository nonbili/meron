package jp.nonbili.meron.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.selection.selectable
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowDropDown
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Translate
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import jp.nonbili.meron.shared.MessageBody
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.launch

internal val LocalMessageTranslations = staticCompositionLocalOf<MessageTranslations?> { null }
internal val LocalMailTranslationLocale = staticCompositionLocalOf { "" }
internal val LocalMailTranslationPrefs = staticCompositionLocalOf<AppPreferences?> { null }

// A fence the core writes around preformatted HTML when it makes the plain body.
private val codeFenceLine = Regex("""(`{3,}|~{3,})[\w-]*""")

/** The text a translation is made from: the reply, without its quote, as the reader sees it. */
internal fun MessageBody.translatableText(): String {
    val quoted = splitQuotedBody(body, bodyQuoteStart, plainBodyLinks(body, bodyLinks))
    return parseInlineMessageText(quoted.reply, quoted.replyLinks)
        .text
        .lines()
        .filterNot { codeFenceLine.matches(it.trim()) }
        .joinToString("\n")
        .trim()
}

@Composable
internal fun MessageTranslationScope(
    content: @Composable () -> Unit,
) {
    val translator = LocalPlatformServices.current.mailTranslator
    val appLocale = LocalMailTranslationLocale.current
    val prefs = LocalMailTranslationPrefs.current
    val scope = rememberCoroutineScope()
    val translations = remember(translator, appLocale, prefs) { translator?.let { MessageTranslations(it, appLocale, prefs, scope) } }
    DisposableEffect(translations) {
        onDispose { translations?.cancel() }
    }
    CompositionLocalProvider(LocalMessageTranslations provides translations, content = content)
}

/** A finished translation of a message into [target]; [unchanged] when it was already in it. */
internal data class MessageTranslationResult(
    val text: String,
    val target: MailTranslationLanguage,
    val unchanged: Boolean,
)

internal data class MessageTranslationState(
    val original: String,
    val quoteStart: Int?,
    val languages: List<MailTranslationLanguage> = emptyList(),
    /** The language asked for: the one being translated into while [loading]. */
    val target: MailTranslationLanguage? = null,
    /** The last finished translation, kept on screen while another language loads. */
    val result: MessageTranslationResult? = null,
    val showOriginal: Boolean = false,
    val loading: Boolean = true,
    val downloading: Boolean = false,
    val failed: Boolean = false,
    val problem: MailTranslationProblem? = null,
) {
    val translatedBody: String? get() = result?.takeUnless { showOriginal || it.unchanged }?.text
}

/** Shared by the chat, traditional layout and full-screen reader of one conversation. */
internal class MessageTranslations(
    private val translator: MailTranslator,
    private val appLocale: String,
    private val prefs: AppPreferences?,
    private val scope: CoroutineScope,
) {
    private val states = mutableStateMapOf<String, MessageTranslationState>()
    private val jobs = mutableMapOf<String, Job>()
    private var languages = emptyList<MailTranslationLanguage>()

    fun state(message: MessageBody): MessageTranslationState? =
        states[message.id]?.takeIf {
            !message.bodyMissing && it.original == message.body && it.quoteStart == message.bodyQuoteStart
        }

    fun toggle(message: MessageBody) {
        val current = state(message)
        when {
            current?.loading == true -> {}

            current?.failed == true -> {
                translate(message, current.target)
            }

            current?.result != null -> {
                states[message.id] = current.copy(showOriginal = !current.showOriginal)
            }

            else -> {
                translate(message, null)
            }
        }
    }

    /** Translates into [target], or the remembered language when null. */
    fun translate(
        message: MessageBody,
        target: MailTranslationLanguage?,
        allowMeteredDownload: Boolean = false,
    ) {
        if (message.bodyMissing) return
        val text = message.translatableText()
        if (text.isBlank()) return
        val id = message.id
        jobs[id]?.cancel()
        val previous = state(message)
        val pending =
            MessageTranslationState(
                original = message.body,
                quoteStart = message.bodyQuoteStart,
                languages = languages,
                target = target,
                result = previous?.result,
                showOriginal = previous?.showOriginal == true,
            )
        states[id] = pending
        jobs[id] =
            scope.launch {
                var current = pending
                try {
                    val available = languages.ifEmpty { translator.languages(appLocale) }
                    ensureActive()
                    languages = available
                    val selected = target ?: rememberedTarget(available)
                    if (selected == null) {
                        states.remove(id)
                        return@launch
                    }
                    current = current.copy(languages = available, target = selected)
                    states[id] = current
                    val translation =
                        translator.translate(text, selected.tag, allowMeteredDownload) {
                            val waiting = current
                            scope.launch { if (states[id] == waiting) states[id] = waiting.copy(downloading = true) }
                        }
                    ensureActive()
                    val unchanged = translation.text.trim() == text.trim()
                    states[id] =
                        current.copy(
                            result = MessageTranslationResult(translation.text, selected, unchanged),
                            showOriginal = false,
                            loading = false,
                            downloading = false,
                        )
                } catch (cancelled: CancellationException) {
                    throw cancelled
                } catch (error: Exception) {
                    ensureActive()
                    states[id] =
                        current.copy(
                            loading = false,
                            downloading = false,
                            failed = true,
                            problem = (error as? MailTranslationException)?.problem,
                        )
                }
            }
    }

    /** Picks [language] for this message and for the ones translated after it. */
    fun choose(
        message: MessageBody,
        language: MailTranslationLanguage,
    ) {
        prefs?.putString(MAIL_TRANSLATION_TARGET_PREF, language.tag)
        translate(message, language)
    }

    /** Stops a translation in progress and goes back to what was shown before it. */
    fun cancel(message: MessageBody) {
        jobs.remove(message.id)?.cancel()
        val current = state(message) ?: return
        val result = current.result
        if (result == null) {
            states.remove(message.id)
        } else {
            states[message.id] = current.copy(target = result.target, loading = false, downloading = false, failed = false, problem = null)
        }
    }

    fun cancel() {
        jobs.values.forEach { it.cancel() }
        jobs.clear()
    }

    private fun rememberedTarget(available: List<MailTranslationLanguage>): MailTranslationLanguage? {
        val remembered = prefs?.getString(MAIL_TRANSLATION_TARGET_PREF, "").orEmpty()
        return available.firstOrNull { it.tag == remembered } ?: defaultMailTranslationTarget(available, appLocale)
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun MessageTranslationControls(
    message: MessageBody,
    state: MessageTranslationState,
    translations: MessageTranslations,
    textColor: Color,
    modifier: Modifier = Modifier,
) {
    var chooseLanguage by remember(message.id) { mutableStateOf(false) }
    val statusStyle = MaterialTheme.typography.bodySmall
    Column(modifier) {
        FlowRow(
            horizontalArrangement = Arrangement.spacedBy(4.dp),
            verticalArrangement = Arrangement.Center,
            itemVerticalAlignment = Alignment.CenterVertically,
        ) {
            state.target?.let { target ->
                val label = tr("chat.translation.language")
                TextButton(
                    onClick = { chooseLanguage = true },
                    modifier = Modifier.semantics { contentDescription = "$label: ${target.name}" },
                ) {
                    Icon(Icons.Filled.Translate, contentDescription = null, modifier = Modifier.size(18.dp))
                    Spacer(Modifier.width(6.dp))
                    Text(target.name)
                    Icon(Icons.Filled.ArrowDropDown, contentDescription = null)
                }
            }
            when {
                state.loading -> {
                    Text(
                        tr(if (state.downloading) "chat.translation.downloading" else "chat.translation.loading"),
                        color = textColor,
                        style = statusStyle,
                        modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite },
                    )
                    TextButton(onClick = { translations.cancel(message) }) {
                        Text(tr("buttons.cancel"))
                    }
                }

                state.failed -> {
                    if (state.problem == MailTranslationProblem.MeteredDownload) {
                        TextButton(onClick = { translations.translate(message, state.target, allowMeteredDownload = true) }) {
                            Text(tr("chat.translation.download"))
                        }
                    } else {
                        TextButton(onClick = { translations.translate(message, state.target) }) {
                            Text(tr("chat.retry"))
                        }
                    }
                    // Back to the translation shown before, or to the original.
                    TextButton(onClick = { translations.cancel(message) }) {
                        Text(tr("buttons.cancel"))
                    }
                }

                state.result?.unchanged == false -> {
                    TextButton(onClick = { translations.toggle(message) }) {
                        Text(tr(if (state.showOriginal) "chat.translation.showTranslation" else "chat.actions.showOriginal"))
                    }
                }
            }
        }
        val note =
            when {
                state.loading -> {
                    null
                }

                state.failed -> {
                    when (state.problem) {
                        MailTranslationProblem.Offline -> tr("chat.translation.offline")
                        MailTranslationProblem.MeteredDownload -> tr("chat.translation.metered")
                        MailTranslationProblem.UnknownLanguage -> tr("chat.translation.unknownLanguage")
                        null -> tr("chat.translation.failed")
                    }
                }

                state.result?.unchanged == true -> {
                    trf("chat.translation.alreadyIn", state.result.target.name)
                }

                else -> {
                    null
                }
            }
        note?.let {
            Text(
                it,
                color = textColor,
                style = statusStyle,
                modifier = Modifier.padding(horizontal = 12.dp).semantics { liveRegion = LiveRegionMode.Polite },
            )
        }
    }
    if (chooseLanguage) {
        TranslationLanguageDialog(
            languages = state.languages,
            selected = state.target,
            onSelect = { language ->
                chooseLanguage = false
                if (language != state.target || state.failed) translations.choose(message, language)
            },
            onDismiss = { chooseLanguage = false },
        )
    }
}

@Composable
private fun TranslationLanguageDialog(
    languages: List<MailTranslationLanguage>,
    selected: MailTranslationLanguage?,
    onSelect: (MailTranslationLanguage) -> Unit,
    onDismiss: () -> Unit,
) {
    var query by remember { mutableStateOf("") }
    val shown =
        remember(languages, query) {
            val needle = query.trim()
            if (needle.isEmpty()) languages else languages.filter { it.name.contains(needle, ignoreCase = true) || it.tag.equals(needle, ignoreCase = true) }
        }
    // Opens on the current language rather than the top of the list.
    val listState = rememberLazyListState(initialFirstVisibleItemIndex = (languages.indexOf(selected) - 2).coerceAtLeast(0))
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(tr("chat.translation.language")) },
        text = {
            Column {
                OutlinedTextField(
                    value = query,
                    onValueChange = { query = it },
                    singleLine = true,
                    placeholder = { Text(tr("chat.translation.searchLanguages")) },
                    leadingIcon = { Icon(Icons.Filled.Search, contentDescription = null) },
                    modifier = Modifier.fillMaxWidth(),
                )
                LazyColumn(Modifier.heightIn(max = 360.dp).padding(top = 8.dp), state = listState) {
                    items(shown, key = { it.tag }) { language ->
                        val current = language == selected
                        Row(
                            Modifier
                                .fillMaxWidth()
                                .selectable(selected = current, role = Role.RadioButton, onClick = { onSelect(language) })
                                .padding(horizontal = 4.dp, vertical = 12.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Text(language.name, Modifier.weight(1f), style = MaterialTheme.typography.bodyLarge)
                            if (current) {
                                Icon(Icons.Filled.Check, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
                            }
                        }
                    }
                }
            }
        },
        confirmButton = {},
        dismissButton = {
            TextButton(onClick = onDismiss) { Text(tr("buttons.cancel")) }
        },
    )
}
