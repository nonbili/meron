package jp.nonbili.meron.ui

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.consumeWindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.input.OutputTransformation
import androidx.compose.foundation.text.input.TextFieldBuffer
import androidx.compose.foundation.text.input.TextFieldLineLimits
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.ContentPaste
import androidx.compose.material.icons.filled.Inbox
import androidx.compose.material.icons.filled.PersonAdd
import androidx.compose.material.icons.filled.RssFeed
import androidx.compose.material.icons.filled.Visibility
import androidx.compose.material.icons.filled.VisibilityOff
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LeadingIconTab
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SecondaryTabRow
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import jp.nonbili.meron.ui.resources.Res
import jp.nonbili.meron.ui.resources.ic_google_g
import kotlinx.coroutines.launch
import org.jetbrains.compose.resources.painterResource

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun AddAccountScreen(
    onBack: () -> Unit,
    initialSection: Int,
    displayName: String,
    onDisplayNameChange: (String) -> Unit,
    senderName: String,
    onSenderNameChange: (String) -> Unit,
    email: String,
    onEmailChange: (String) -> Unit,
    username: String,
    onUsernameChange: (String) -> Unit,
    password: String,
    onPasswordChange: (String) -> Unit,
    host: String,
    onHostChange: (String) -> Unit,
    imapPort: String,
    onImapPortChange: (String) -> Unit,
    smtpHost: String,
    onSmtpHostChange: (String) -> Unit,
    smtpPort: String,
    onSmtpPortChange: (String) -> Unit,
    serverSettingsOpen: Boolean,
    onServerSettingsOpenChange: (Boolean) -> Unit,
    onAutodiscover: () -> Unit,
    onEmailBlur: () -> Unit,
    onAddPassword: () -> Unit,
    oauthAuthorizationCode: String,
    onLaunchOAuth: () -> Unit,
    onConnectGoogleDeviceAccount: () -> Unit,
    rssFeedUrl: String,
    onRssFeedUrlChange: (String) -> Unit,
    rssDisplayName: String,
    onRssDisplayNameChange: (String) -> Unit,
    rssAccountAdding: Boolean,
    onAddRss: () -> Unit,
) {
    val pagerState = rememberPagerState(initialPage = initialSection) { 3 }
    val coroutineScope = rememberCoroutineScope()
    val tabs =
        listOf(
            tr("accounts.setup.oauthTab"),
            tr("accounts.setup.passwordTab"),
            tr("accounts.setup.rssTab"),
        )
    val icons =
        listOf(
            Icons.Default.PersonAdd,
            Icons.Default.Inbox,
            Icons.Default.RssFeed,
        )

    LaunchedEffect(initialSection) {
        pagerState.scrollToPage(initialSection)
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(tr("accounts.actions.addAccount")) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = tr("buttons.back"))
                    }
                },
            )
        },
    ) { innerPadding ->
        Column(
            modifier =
                Modifier
                    .fillMaxSize()
                    .padding(innerPadding)
                    .consumeWindowInsets(innerPadding),
        ) {
            SecondaryTabRow(
                selectedTabIndex = pagerState.currentPage,
                modifier = Modifier.fillMaxWidth(),
            ) {
                tabs.forEachIndexed { index, label ->
                    LeadingIconTab(
                        selected = pagerState.currentPage == index,
                        onClick = {
                            coroutineScope.launch {
                                pagerState.animateScrollToPage(index)
                            }
                        },
                        text = { Text(label, maxLines = 1) },
                        icon = { Icon(icons[index], contentDescription = label) },
                        selectedContentColor = MaterialTheme.colorScheme.primary,
                        unselectedContentColor = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }

            HorizontalPager(
                state = pagerState,
                modifier =
                    Modifier
                        .fillMaxSize()
                        .weight(1f)
                        .imePadding(),
            ) { page ->
                LazyColumn(
                    modifier = Modifier.fillMaxSize(),
                    contentPadding = PaddingValues(16.dp),
                    verticalArrangement = Arrangement.spacedBy(16.dp),
                ) {
                    when (page) {
                        0 -> {
                            item {
                                SetupCard(title = "") {
                                    Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                                        OAuthSignInButton(
                                            label = trf("accounts.oauth.signInWithProvider", tr("accounts.providers.googleName")),
                                            provider = "google",
                                            onClick = onConnectGoogleDeviceAccount,
                                        )
                                        OAuthSignInButton(
                                            label = trf("accounts.oauth.signInWithProvider", tr("accounts.providers.outlookName")),
                                            provider = "outlook",
                                            onClick = onLaunchOAuth,
                                        )
                                    }
                                    if (oauthAuthorizationCode.isNotBlank()) {
                                        Text(
                                            tr("accounts.oauth.finishingSignIn"),
                                            style = MaterialTheme.typography.bodySmall,
                                            color = MaterialTheme.colorScheme.primary,
                                        )
                                    }
                                }
                            }
                        }

                        1 -> {
                            item {
                                SetupCard(title = "") {
                                    SetupField(displayName, onDisplayNameChange, tr("accounts.fields.displayNameMeronOnly"))
                                    SetupField(senderName, onSenderNameChange, tr("accounts.fields.senderNameOutgoing"))
                                    SetupField(email, onEmailChange, tr("accounts.fields.emailAddress"), onFocusLost = onEmailBlur)
                                    SetupField(password, onPasswordChange, tr("accounts.fields.password"), isPassword = true)
                                    OutlinedButton(onClick = onAutodiscover, modifier = Modifier.fillMaxWidth()) {
                                        Text(tr("mobile.accounts.findMailSettings"))
                                    }
                                    TextButton(
                                        onClick = { onServerSettingsOpenChange(!serverSettingsOpen) },
                                        modifier = Modifier.align(Alignment.Start),
                                    ) {
                                        Text(tr("accounts.advancedServerSettings"))
                                    }
                                    if (serverSettingsOpen) {
                                        SetupField(username, onUsernameChange, tr("accounts.fields.username"))
                                        SetupField(host, onHostChange, tr("accounts.fields.imapHost"))
                                        SetupField(imapPort, onImapPortChange, tr("accounts.fields.imapPort"))
                                        SetupField(smtpHost, onSmtpHostChange, tr("accounts.fields.smtpHost"))
                                        SetupField(smtpPort, onSmtpPortChange, tr("accounts.fields.smtpPort"))
                                    }
                                    Button(onClick = onAddPassword, modifier = Modifier.fillMaxWidth()) { Text(tr("accounts.actions.addAccount")) }
                                }
                            }
                        }

                        else -> {
                            item {
                                SetupCard(title = "") {
                                    SetupField(rssDisplayName, onRssDisplayNameChange, tr("accounts.fields.accountName"))
                                    SetupField(rssFeedUrl, onRssFeedUrlChange, tr("accounts.fields.firstFeedUrl"))
                                    Text(
                                        tr("accounts.setup.feedAccountHint"),
                                        style = MaterialTheme.typography.bodySmall,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    )
                                    Button(
                                        onClick = onAddRss,
                                        enabled = rssDisplayName.isNotBlank() && !rssAccountAdding,
                                        modifier = Modifier.fillMaxWidth(),
                                    ) {
                                        if (rssAccountAdding) {
                                            CircularProgressIndicator(
                                                modifier = Modifier.size(20.dp),
                                                strokeWidth = 2.dp,
                                                color = MaterialTheme.colorScheme.onPrimary,
                                            )
                                        } else {
                                            Text(tr("accounts.actions.saveAccount"))
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
internal fun OAuthSignInButton(
    label: String,
    provider: String,
    onClick: () -> Unit,
) {
    Surface(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(14.dp),
        color = MaterialTheme.colorScheme.surface,
        border = androidx.compose.foundation.BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
        onClick = onClick,
    ) {
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 18.dp, vertical = 13.dp),
            horizontalArrangement = Arrangement.Center,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            OAuthBrandIcon(provider = provider, size = 20.dp)
            Spacer(Modifier.width(12.dp))
            Text(label, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
        }
    }
}

@Composable
internal fun OAuthBrandIcon(
    provider: String,
    size: Dp,
) {
    if (provider == "outlook") {
        Column(Modifier.size(size), verticalArrangement = Arrangement.spacedBy(1.dp)) {
            Row(Modifier.weight(1f), horizontalArrangement = Arrangement.spacedBy(1.dp)) {
                Box(Modifier.weight(1f).fillMaxSize().background(Color(0xFFF25022)))
                Box(Modifier.weight(1f).fillMaxSize().background(Color(0xFF7FBA00)))
            }
            Row(Modifier.weight(1f), horizontalArrangement = Arrangement.spacedBy(1.dp)) {
                Box(Modifier.weight(1f).fillMaxSize().background(Color(0xFF00A4EF)))
                Box(Modifier.weight(1f).fillMaxSize().background(Color(0xFFFFB900)))
            }
        }
    } else {
        Image(
            painter = painterResource(Res.drawable.ic_google_g),
            contentDescription = null,
            modifier = Modifier.size(size),
        )
    }
}

@Composable
internal fun SetupCard(
    title: String,
    content: @Composable ColumnScope.() -> Unit,
) {
    Card(Modifier.fillMaxWidth(), colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer)) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            if (title.isNotBlank()) {
                Text(title, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
            }
            content()
        }
    }
}

@Composable
internal fun SetupField(
    value: String,
    onChange: (String) -> Unit,
    label: String,
    isPassword: Boolean = false,
    placeholder: String? = null,
    onFocusLost: (() -> Unit)? = null,
) {
    // Compose Multiplatform on iOS doesn't show the long-press paste menu while a
    // PasswordVisualTransformation is active, so reveal the password by default
    // there; the eye toggle still lets the user hide it. Android keeps it masked.
    var revealed by remember { mutableStateOf(isPassword && maskPasswordsByDefault.not()) }
    var wasFocused by remember { mutableStateOf(false) }

    // Use TextFieldState so that nativeTextKeyboardOptions (usingNativeTextInput
    // on iOS) actually takes effect – the value/onValueChange overload ignores it.
    val textFieldState = rememberTextFieldState(initialText = value)

    // Sync external value → internal state (e.g. when the parent clears the form).
    LaunchedEffect(value) {
        if (textFieldState.text.toString() != value) {
            textFieldState.edit { replace(0, length, value) }
        }
    }

    // Propagate internal edits → external onChange callback.
    LaunchedEffect(textFieldState) {
        snapshotFlow { textFieldState.text.toString() }
            .collect { newText ->
                if (newText != value) onChange(newText)
            }
    }

    OutlinedTextField(
        state = textFieldState,
        label = { Text(label) },
        placeholder = placeholder?.let { { Text(it) } },
        lineLimits = TextFieldLineLimits.SingleLine,
        keyboardOptions = nativeTextKeyboardOptions,
        modifier =
            Modifier.fillMaxWidth().let { m ->
                if (onFocusLost != null) {
                    m.onFocusChanged { focusState ->
                        if (wasFocused && !focusState.isFocused) onFocusLost()
                        wasFocused = focusState.isFocused
                    }
                } else {
                    m
                }
            },
        outputTransformation = if (isPassword && !revealed) PasswordOutputTransformation else null,
        trailingIcon =
            if (isPassword || value.isEmpty()) {
                {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(4.dp),
                    ) {
                        val clipboardManager = LocalClipboardManager.current
                        if (value.isEmpty()) {
                            IconButton(onClick = {
                                val text = clipboardManager.getText()?.text.orEmpty()
                                if (text.isNotEmpty()) {
                                    onChange(text)
                                }
                            }) {
                                Icon(
                                    Icons.Filled.ContentPaste,
                                    contentDescription = "Paste",
                                    modifier = Modifier.size(20.dp),
                                )
                            }
                        }
                        if (isPassword) {
                            IconButton(onClick = { revealed = !revealed }) {
                                Icon(
                                    if (revealed) Icons.Filled.VisibilityOff else Icons.Filled.Visibility,
                                    contentDescription = if (revealed) "Hide password" else "Show password",
                                    modifier = Modifier.size(20.dp),
                                )
                            }
                        }
                    }
                }
            } else {
                null
            },
    )
}

/**
 * [OutputTransformation] that masks every character with a bullet (●),
 * equivalent to [PasswordVisualTransformation] but for the TextFieldState API.
 */
private object PasswordOutputTransformation : OutputTransformation {
    override fun TextFieldBuffer.transformOutput() {
        val masked = "●".repeat(length)
        replace(0, length, masked)
    }
}
