package jp.nonbili.meron.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.heightIn
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.MaterialTheme
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
import androidx.compose.ui.unit.dp
import jp.nonbili.meron.shared.MessageBody
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.launch

internal val LocalMessageTranslations = staticCompositionLocalOf<MessageTranslations?> { null }
internal val LocalMailTranslationLocale = staticCompositionLocalOf { "" }

internal fun MessageBody.translatableText(): String = splitQuotedBody(body, bodyQuoteStart).reply.trim()

@Composable
internal fun MessageTranslationScope(
    content: @Composable () -> Unit,
) {
    val translator = LocalPlatformServices.current.mailTranslator
    val appLocale = LocalMailTranslationLocale.current
    val scope = rememberCoroutineScope()
    val translations = remember(translator, appLocale) { translator?.let { MessageTranslations(it, appLocale, scope) } }
    DisposableEffect(translations) {
        onDispose { translations?.cancel() }
    }
    CompositionLocalProvider(LocalMessageTranslations provides translations, content = content)
}

internal data class MessageTranslationState(
    val original: String,
    val quoteStart: Int?,
    val languages: List<MailTranslationLanguage> = emptyList(),
    val target: MailTranslationLanguage? = null,
    val result: MailTranslation? = null,
    val showOriginal: Boolean = false,
    val loading: Boolean = true,
) {
    val translatedBody: String? get() = if (showOriginal) null else result?.text
}

/** Shared by the chat, traditional layout and full-screen reader of one conversation. */
internal class MessageTranslations(
    private val translator: MailTranslator,
    private val appLocale: String,
    private val scope: CoroutineScope,
) {
    private val states = mutableStateMapOf<String, MessageTranslationState>()
    private val jobs = mutableMapOf<String, Job>()
    private var languages = emptyList<MailTranslationLanguage>()

    val attribution: String? get() = translator.attribution

    fun state(message: MessageBody): MessageTranslationState? =
        states[message.id]?.takeIf {
            !message.bodyMissing && it.original == message.body && it.quoteStart == message.bodyQuoteStart
        }

    fun toggle(message: MessageBody) {
        val current = state(message)
        when {
            current?.result != null -> states[message.id] = current.copy(showOriginal = !current.showOriginal)
            current?.loading != true -> translate(message, current?.target)
        }
    }

    fun translate(
        message: MessageBody,
        target: MailTranslationLanguage?,
    ) {
        if (message.bodyMissing) return
        val text = message.translatableText()
        if (text.isBlank()) return
        val id = message.id
        jobs[id]?.cancel()
        val pending = MessageTranslationState(message.body, message.bodyQuoteStart, languages, target)
        states[id] = pending
        jobs[id] =
            scope.launch {
                var current = pending
                try {
                    val available = languages.ifEmpty { translator.languages().sortedBy { it.name.lowercase() } }
                    ensureActive()
                    languages = available
                    val selected = target ?: defaultMailTranslationTarget(available, appLocale)
                    current = pending.copy(languages = available, target = selected)
                    states[id] = current
                    val result = selected?.let { translator.translate(text, it.tag) }
                    ensureActive()
                    states[id] = current.copy(result = result, loading = false)
                } catch (cancelled: CancellationException) {
                    throw cancelled
                } catch (_: Exception) {
                    ensureActive()
                    states[id] = current.copy(loading = false)
                }
            }
    }

    fun cancel() {
        jobs.values.forEach { it.cancel() }
        jobs.clear()
    }
}

@Composable
internal fun MessageTranslationControls(
    message: MessageBody,
    state: MessageTranslationState,
    translations: MessageTranslations,
    textColor: Color,
    modifier: Modifier = Modifier,
) {
    var chooseLanguage by remember(message.id) { mutableStateOf(false) }
    Column(modifier) {
        Row(
            horizontalArrangement = Arrangement.spacedBy(4.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            when {
                state.loading -> {
                    Text(tr("chat.translation.loading"), color = textColor, style = MaterialTheme.typography.bodySmall)
                }

                state.result != null -> {
                    TextButton(onClick = { translations.toggle(message) }, modifier = Modifier.weight(1f, fill = false)) {
                        Text(tr(if (state.showOriginal) "chat.translation.showTranslation" else "chat.actions.showOriginal"))
                    }
                }

                else -> {
                    TextButton(onClick = { translations.toggle(message) }, modifier = Modifier.weight(1f, fill = false)) {
                        Text(tr("chat.retry"))
                    }
                }
            }
            state.target?.let { target ->
                Box(Modifier.weight(1f, fill = false)) {
                    TextButton(onClick = { chooseLanguage = true }) {
                        Text(trf("chat.translation.target", target.name))
                    }
                    DropdownMenu(
                        expanded = chooseLanguage,
                        onDismissRequest = { chooseLanguage = false },
                        modifier = Modifier.heightIn(max = 280.dp),
                    ) {
                        state.languages.forEach { language ->
                            DropdownMenuItem(
                                text = { Text(language.name) },
                                onClick = {
                                    chooseLanguage = false
                                    if (language != target) translations.translate(message, language)
                                },
                            )
                        }
                    }
                }
            }
        }
        if (!state.loading && state.result == null) {
            Text(tr("chat.translation.failed"), color = textColor, style = MaterialTheme.typography.bodySmall)
        }
        if (state.translatedBody != null) {
            translations.attribution?.let {
                Text(it, color = textColor, style = MaterialTheme.typography.labelSmall)
            }
        }
    }
}
