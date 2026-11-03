package jp.nonbili.meron.ui

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.Person
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.isSpecified
import androidx.compose.ui.unit.sp
import jp.nonbili.meron.shared.ContactSuggestion
import jp.nonbili.meron.shared.MessageBody

internal val LocalOpenContact = staticCompositionLocalOf<((ContactSuggestion) -> Unit)?> { null }

// Sender and recipients as tappable chips — the mobile counterpart of the
// desktop bubble's address popover. A tap copies the full address; a long press
// opens the per-address actions.
@Composable
internal fun MessageAddressDetails(
    message: MessageBody,
    onCopy: (String, String) -> Unit,
    onComposeTo: (String) -> Unit,
    textColor: Color,
    modifier: Modifier = Modifier,
    // A fixed label column lines the chips of every row up, for the reader.
    labelWidth: Dp = Dp.Unspecified,
) {
    val fromLabel = tr("composer.fields.from")
    val toLabel = tr("composer.fields.to")
    val ccLabel = tr("composer.fields.cc")
    val bccLabel = tr("composer.fields.bcc")
    val fromRaw = remember(message.from, message.fromAddr) { fullFromAddress(message) }
    val replyToDiffers =
        remember(message.replyTo, message.fromAddr) {
            message.replyTo.isNotBlank() &&
                !parseAddressList(message.replyTo)
                    .firstOrNull()
                    ?.second
                    .orEmpty()
                    .equals(message.fromAddr.trim(), ignoreCase = true)
        }
    Column(modifier, verticalArrangement = Arrangement.spacedBy(4.dp)) {
        MessageAddressRow(fromLabel, fromRaw, onCopy, onComposeTo, textColor, labelWidth)
        MessageAddressRow(toLabel, message.to, onCopy, onComposeTo, textColor, labelWidth)
        MessageAddressRow(ccLabel, message.cc, onCopy, onComposeTo, textColor, labelWidth)
        MessageAddressRow(bccLabel, message.bcc, onCopy, onComposeTo, textColor, labelWidth)
        if (replyToDiffers) MessageAddressRow("Reply-To", message.replyTo, onCopy, onComposeTo, textColor, labelWidth)
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun MessageAddressRow(
    label: String,
    rawList: String,
    onCopy: (String, String) -> Unit,
    onComposeTo: (String) -> Unit,
    textColor: Color,
    labelWidth: Dp,
) {
    val items = remember(rawList) { addressChipItems(rawList) }
    if (items.isEmpty()) return
    Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        Text(
            "${label.uppercase()}:",
            modifier = Modifier.padding(top = 4.dp).then(if (labelWidth.isSpecified) Modifier.width(labelWidth) else Modifier),
            fontSize = 9.sp,
            fontWeight = FontWeight.SemiBold,
            color = textColor.copy(alpha = 0.6f),
        )
        FlowRow(
            horizontalArrangement = Arrangement.spacedBy(4.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            items.forEach { item ->
                AddressChip(
                    label = label,
                    item = item,
                    onCopy = onCopy,
                    onComposeTo = onComposeTo,
                    textColor = textColor,
                )
            }
        }
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun AddressChip(
    label: String,
    item: AddressChipItem,
    onCopy: (String, String) -> Unit,
    onComposeTo: (String) -> Unit,
    textColor: Color,
) {
    var menuOpen by remember(item.full) { mutableStateOf(false) }
    val emailLabel = tr("accounts.fields.emailAddress")
    val email = item.email.ifBlank { item.display }
    val openContact = LocalOpenContact.current
    Box {
        Surface(
            shape = RoundedCornerShape(6.dp),
            color = MaterialTheme.colorScheme.primary.copy(alpha = 0.08f),
            modifier =
                Modifier.combinedClickable(
                    onClick = { onCopy(label, item.full) },
                    onLongClick = { menuOpen = true },
                ),
        ) {
            // A display name alone hides who the message really came from, so
            // the address trails it on the same line.
            Text(
                if (item.email.isBlank()) item.display else "${item.display} · ${item.email}",
                modifier = Modifier.padding(horizontal = 6.dp, vertical = 3.dp),
                fontSize = 11.sp,
                color = textColor,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        DropdownMenu(expanded = menuOpen, onDismissRequest = { menuOpen = false }) {
            if (item.email.isNotBlank()) {
                DropdownMenuItem(
                    text = { Text(tr("chat.copyFullAddress")) },
                    leadingIcon = { Icon(Icons.Filled.ContentCopy, contentDescription = null) },
                    onClick = {
                        menuOpen = false
                        onCopy(label, item.full)
                    },
                )
            }
            DropdownMenuItem(
                text = { Text(tr("chat.copyEmailAddress")) },
                leadingIcon = { Icon(Icons.Filled.ContentCopy, contentDescription = null) },
                onClick = {
                    menuOpen = false
                    onCopy(emailLabel, email)
                },
            )
            if (email.contains("@")) {
                if (openContact != null) {
                    DropdownMenuItem(
                        text = { Text(tr("contacts.open")) },
                        leadingIcon = { Icon(Icons.Filled.Person, contentDescription = null) },
                        onClick = {
                            menuOpen = false
                            openContact(ContactSuggestion(name = item.display, addr = email))
                        },
                    )
                }
                DropdownMenuItem(
                    text = { Text(tr("composer.actions.newMessage")) },
                    leadingIcon = { Icon(Icons.Filled.Edit, contentDescription = null) },
                    onClick = {
                        menuOpen = false
                        onComposeTo(email)
                    },
                )
            }
        }
    }
}
