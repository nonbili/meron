package jp.nonbili.meron.ui

import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.SnackbarHost
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.ExperimentalComposeUiApi
import kotlin.io.encoding.ExperimentalEncodingApi

@OptIn(ExperimentalMaterial3Api::class, ExperimentalEncodingApi::class, ExperimentalComposeUiApi::class)
@Composable
internal fun AddAccountRouteContent(
    state: MeronMobileState,
    popAppBack: () -> Unit,
) {
    with(state) {
        LaunchedEffect(Unit) {
            if (rssDisplayName.isBlank()) {
                rssDisplayName = nextRssAccountDisplayName(coreAccounts)
            }
        }
        AddAccountScreen(
            snackbarHost = { SnackbarHost(snackbarHost) },
            onBack = popAppBack,
            initialSection = addSection,
            // Reconnect pre-fills the address of an account core can no
            // longer sign in to; any other address is a new account.
            reconnecting = coreAccounts.any { it.needsReconnect && it.email.equals(email.trim(), ignoreCase = true) },
            displayName = displayName,
            onDisplayNameChange = { displayName = it },
            senderName = senderName,
            onSenderNameChange = { senderName = it },
            email = email,
            onEmailChange = { email = it },
            username = username,
            onUsernameChange = { username = it },
            password = password,
            onPasswordChange = { password = it },
            host = host,
            onHostChange = {
                host = it
                hostTouched = it.isNotBlank()
            },
            imapPort = imapPort,
            onImapPortChange = {
                imapPort = it
                imapSecurity = mailSecurityAfterPortEdit(imapSecurity, imapSecurityTouched, it)
                imapPortTouched = it.isNotBlank()
            },
            imapSecurity = imapSecurity,
            onImapSecurityChange = {
                imapSecurity = it
                imapSecurityTouched = true
            },
            smtpHost = smtpHost,
            onSmtpHostChange = {
                smtpHost = it
                smtpHostTouched = it.isNotBlank()
            },
            smtpPort = smtpPort,
            onSmtpPortChange = {
                smtpPort = it
                smtpSecurity = mailSecurityAfterPortEdit(smtpSecurity, smtpSecurityTouched, it)
                smtpPortTouched = it.isNotBlank()
            },
            smtpSecurity = smtpSecurity,
            onSmtpSecurityChange = {
                smtpSecurity = it
                smtpSecurityTouched = true
            },
            serverSettingsOpen = passwordServerSettingsOpen,
            onServerSettingsOpenChange = { passwordServerSettingsOpen = it },
            onAutodiscover = ::autodiscoverPasswordAccount,
            onEmailBlur = { autodiscoverPasswordAccount(auto = true) },
            onAddPassword = ::addPasswordAccount,
            oauthAuthorizationCode = oauthAuthorizationCode,
            onLaunchOAuth = {
                oauthProvider = "outlook"
                launchOAuthFlow()
            },
            onConnectGoogleDeviceAccount = {
                oauthProvider = "gmail"
                connectGoogleDeviceAccount()
            },
            rssFeedUrl = rssFeedUrl,
            onRssFeedUrlChange = { rssFeedUrl = it },
            rssDisplayName = rssDisplayName,
            onRssDisplayNameChange = { rssDisplayName = it },
            rssAccountAdding = rssAccountAdding,
            onAddRss = ::addRssAccount,
        )
    }
}
