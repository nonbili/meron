import { CalendarDays, CalendarPlus, CalendarX, Check, Clock, HelpCircle, MapPin, Repeat, User, X } from 'lucide-react'
import { useState } from 'react'
import { formatEventRange } from '../../lib/date'
import { useTranslation } from '../../lib/i18n'
import { clsx } from '../../lib/utils'
import { addInviteToCalendar, removeInviteFromCalendar, respondToInvite } from '../../states/calendar'
import type { Invite, InviteResponse, Message } from '../../types'
import { Button } from '../button/Button'

const RESPONSES: { response: InviteResponse; icon: typeof Check; label: string }[] = [
  { response: 'accepted', icon: Check, label: 'invite.accept' },
  { response: 'tentative', icon: HelpCircle, label: 'invite.maybe' },
  { response: 'declined', icon: X, label: 'invite.decline' },
]

const OWN_RESPONSE: Record<InviteResponse, string> = {
  accepted: 'invite.youAccepted',
  tentative: 'invite.youTentative',
  declined: 'invite.youDeclined',
}

const THEIR_RESPONSE: Record<InviteResponse, string> = {
  accepted: 'invite.replyAccepted',
  tentative: 'invite.replyTentative',
  declined: 'invite.replyDeclined',
}

/** On a reply: who answered, and how — the user, on the copy in Sent. */
function replyLine(invite: Invite, outgoing: boolean): { key: string; name: string } | null {
  if (invite.method !== 'reply') return null
  const attendee = invite.attendees[0]
  const partstat = attendee?.partstat as InviteResponse
  const key = attendee && (outgoing ? OWN_RESPONSE : THEIR_RESPONSE)[partstat]
  return key ? { key, name: attendee.name || attendee.addr } : null
}

// The event a calendar invitation carries, above the message body, with
// Accept / Maybe / Decline when the invitation is open. Mirrors mobile's
// InviteCard.
export function InviteCard({ message, invite }: { message: Message; invite: Invite }) {
  const { t } = useTranslation()
  const [sending, setSending] = useState(false)
  const reply = replyLine(invite, !!message.outgoing)
  const organizer = invite.organizer.name || invite.organizer.addr

  const respond = async (response: InviteResponse) => {
    setSending(true)
    try {
      await respondToInvite(message, response)
    } finally {
      setSending(false)
    }
  }

  return (
    <div className="mb-2 w-full max-w-[480px] rounded-xl border border-border/20 bg-black/[0.03] p-3 text-xs dark:bg-white/[0.03]">
      <div className="flex items-start gap-2">
        <CalendarDays size={15} className="mt-px shrink-0 text-accent" />
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-1.5">
            <span className={clsx('font-semibold break-words select-text', invite.cancelled && 'line-through')}>
              {invite.summary || t('invite.untitled')}
            </span>
            {invite.cancelled && (
              <span className="shrink-0 rounded-md bg-rose-50 px-1.5 py-px text-[0.625rem] font-semibold text-rose-600 dark:bg-rose-950/30 dark:text-rose-400">
                {t('invite.cancelled')}
              </span>
            )}
          </div>
          <div className="mt-1.5 flex flex-col gap-1 text-secondary">
            <span className="flex items-center gap-1.5">
              <Clock size={12} className="shrink-0" />
              <span className="select-text">
                {formatEventRange(invite.start, invite.end, invite.all_day)}
                {invite.all_day && ` · ${t('invite.allDay')}`}
              </span>
              {invite.recurring && (
                <span className="flex items-center gap-1" title={t('invite.recurring')}>
                  <Repeat size={11} />
                  {t('invite.recurring')}
                </span>
              )}
            </span>
            {invite.location && (
              <span className="flex items-start gap-1.5">
                <MapPin size={12} className="mt-px shrink-0" />
                <span className="break-words select-text">{invite.location}</span>
              </span>
            )}
            {organizer && (
              <span className="flex items-center gap-1.5">
                <User size={12} className="shrink-0" />
                <span className="truncate select-text" title={invite.organizer.addr}>
                  {t('invite.organizer', { name: organizer })}
                </span>
              </span>
            )}
          </div>
          {reply && <div className="mt-2 font-semibold">{t(reply.key, { name: reply.name })}</div>}
          {invite.cancelled && invite.event_id && (
            <Button
              className="mt-2.5"
              size="sm"
              variant="secondary"
              leftIcon={CalendarX}
              onClick={() => void removeInviteFromCalendar(message)}
            >
              {t('calendar.removeFromCalendar')}
            </Button>
          )}
          {invite.can_respond && (
            <div className="mt-2.5 flex flex-wrap items-center gap-1.5">
              {RESPONSES.map(({ response, icon, label }) => (
                <Button
                  key={response}
                  size="sm"
                  variant={invite.response === response ? 'primary' : 'secondary'}
                  leftIcon={icon}
                  disabled={sending}
                  aria-pressed={invite.response === response}
                  onClick={() => invite.response !== response && respond(response)}
                >
                  {t(invite.response === response ? OWN_RESPONSE[response] : label)}
                </Button>
              ))}
              {invite.response && invite.response !== 'declined' && !invite.event_id && (
                <Button
                  size="sm"
                  variant="ghost"
                  leftIcon={CalendarPlus}
                  onClick={() => void addInviteToCalendar(message)}
                >
                  {t('calendar.addToCalendar')}
                </Button>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
