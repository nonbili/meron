package jp.nonbili.meron.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Event
import androidx.compose.material.icons.filled.EventAvailable
import androidx.compose.material.icons.filled.EventBusy
import androidx.compose.material.icons.filled.Person
import androidx.compose.material.icons.filled.Place
import androidx.compose.material.icons.filled.Repeat
import androidx.compose.material.icons.filled.Schedule
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import jp.nonbili.meron.shared.MessageInvite

private val responses =
    listOf(
        "accepted" to "invite.accept",
        "tentative" to "invite.maybe",
        "declined" to "invite.decline",
    )

private val ownResponse =
    mapOf(
        "accepted" to "invite.youAccepted",
        "tentative" to "invite.youTentative",
        "declined" to "invite.youDeclined",
    )

private val theirResponse =
    mapOf(
        "accepted" to "invite.replyAccepted",
        "tentative" to "invite.replyTentative",
        "declined" to "invite.replyDeclined",
    )

/**
 * The event a calendar invitation carries, above the message body, with
 * Accept / Maybe / Decline when the invitation is open. Mirrors desktop's
 * InviteCard.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun InviteCard(
    invite: MessageInvite,
    // A reply in Sent is the user's own answer.
    outgoing: Boolean,
    textColor: Color,
    onRespond: (String) -> Unit,
    onRemoveFromCalendar: () -> Unit,
    onAddToCalendar: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val secondary = textColor.copy(alpha = 0.7f)
    Surface(
        modifier = modifier.fillMaxWidth(),
        shape = RoundedCornerShape(8.dp),
        color = textColor.copy(alpha = 0.08f),
        contentColor = textColor,
    ) {
        Column(Modifier.padding(horizontal = 10.dp, vertical = 9.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Icon(Icons.Filled.Event, contentDescription = null, tint = MaterialTheme.colorScheme.primary, modifier = Modifier.size(18.dp))
                Text(
                    invite.summary.ifBlank { tr("invite.untitled") },
                    modifier = Modifier.weight(1f, fill = false),
                    fontSize = 14.sp,
                    fontWeight = FontWeight.SemiBold,
                    textDecoration = if (invite.cancelled) TextDecoration.LineThrough else null,
                )
                if (invite.cancelled) {
                    Text(tr("invite.cancelled"), fontSize = 11.sp, fontWeight = FontWeight.SemiBold, color = MaterialTheme.colorScheme.error)
                }
            }
            val allDay = if (invite.allDay) " · ${tr("invite.allDay")}" else ""
            InviteDetail(Icons.Filled.Schedule, formatEventRange(invite.start, invite.end, invite.allDay) + allDay, secondary)
            if (invite.recurring) InviteDetail(Icons.Filled.Repeat, tr("invite.recurring"), secondary)
            if (invite.location.isNotBlank()) InviteDetail(Icons.Filled.Place, invite.location, secondary)
            val organizer = invite.organizerName.ifBlank { invite.organizerAddr }
            if (organizer.isNotBlank()) InviteDetail(Icons.Filled.Person, tr("invite.organizer", mapOf("name" to organizer)), secondary)
            if (invite.method == "reply") {
                val attendee = invite.attendees.firstOrNull()
                val key = attendee?.let { (if (outgoing) ownResponse else theirResponse)[it.partstat] }
                if (attendee != null && key != null) {
                    Text(
                        tr(key, mapOf("name" to attendee.name.ifBlank { attendee.addr })),
                        modifier = Modifier.padding(top = 2.dp),
                        fontSize = 13.sp,
                        fontWeight = FontWeight.SemiBold,
                    )
                }
            }
            if (invite.cancelled && invite.eventId.isNotBlank()) {
                TextButton(onClick = onRemoveFromCalendar, contentPadding = PaddingValues(horizontal = 4.dp)) {
                    Icon(Icons.Filled.EventBusy, contentDescription = null, modifier = Modifier.size(16.dp))
                    Text(tr("calendar.removeFromCalendar"), modifier = Modifier.padding(start = 6.dp))
                }
            }
            if (invite.canRespond) {
                FlowRow(
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                    verticalArrangement = Arrangement.Center,
                ) {
                    responses.forEach { (response, label) ->
                        FilterChip(
                            selected = invite.response == response,
                            onClick = { if (invite.response != response) onRespond(response) },
                            label = { Text(tr(if (invite.response == response) ownResponse.getValue(response) else label)) },
                        )
                    }
                }
                // Answered, but its event was deleted since.
                if (invite.response != "" && invite.response != "declined" && invite.eventId.isBlank()) {
                    TextButton(onClick = onAddToCalendar, contentPadding = PaddingValues(horizontal = 4.dp)) {
                        Icon(Icons.Filled.EventAvailable, contentDescription = null, modifier = Modifier.size(16.dp))
                        Text(tr("calendar.addToCalendar"), modifier = Modifier.padding(start = 6.dp))
                    }
                }
            }
        }
    }
}

@Composable
private fun InviteDetail(
    icon: ImageVector,
    text: String,
    color: Color,
) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Icon(icon, contentDescription = null, tint = color, modifier = Modifier.size(14.dp))
        Text(text, fontSize = 12.5.sp, color = color, maxLines = 3, overflow = TextOverflow.Ellipsis)
    }
}
