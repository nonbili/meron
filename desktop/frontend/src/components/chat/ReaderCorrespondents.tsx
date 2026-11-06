import { ChevronDown } from 'lucide-react'
import { type ReactNode, useState } from 'react'
import { useTranslation } from '../../lib/i18n'
import type { Account, MessageTab } from '../../types'
import { Avatar } from '../avatar/Avatar'
import { AddressRow } from './AddressList'
import { extractAddr, formatFullTimestamp, formatRecipientSummary, parseAddressList } from './messageHelpers'

// Who sent the message and when, folded to the sender and a "to …" line like
// the conversation's message headers: the full lists are rarely what the
// reader came for. The chevron unfolds every list, each pill copying its full
// address.
export function ReaderCorrespondents({
  tab,
  account,
  isRSS = false,
  remote,
}: {
  tab: Pick<
    MessageTab,
    'from' | 'fromAddr' | 'fromRaw' | 'to' | 'cc' | 'bcc' | 'replyTo' | 'date' | 'outgoing' | 'feedIcon'
  >
  account?: Account
  isRSS?: boolean
  /** The remote-content affordance, beside the date. */
  remote: ReactNode
}) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  // Suppress Reply-To when it repeats From, as in the conversation header.
  const replyToDiffers =
    !!tab.replyTo && extractAddr(tab.replyTo).toLowerCase() !== extractAddr(tab.fromRaw ?? '').toLowerCase()
  const from = parseAddressList(tab.fromRaw)[0]
  const senderName = from?.name || tab.from
  const senderAddr = from?.email || tab.fromAddr
  const outgoing = !!tab.outgoing
  const recipients = formatRecipientSummary(tab.to, tab.cc)
  // A feed item has no recipients, so the lists could only repeat the feed
  // name the header already shows.
  const canOpen = !isRSS && !!(tab.to || tab.cc || tab.bcc || replyToDiffers)
  const stamp = tab.date ? formatFullTimestamp(tab.date) : ''

  return (
    <div className="mt-3 rounded-xl border border-border/40 bg-chats px-4 py-3 shadow-card">
      <div className="flex items-start gap-2.5">
        <Avatar
          name={(outgoing ? account?.display_name : '') || senderName}
          email={outgoing ? undefined : senderAddr}
          src={outgoing ? account?.avatar_url : tab.feedIcon ? `/media/${tab.feedIcon}` : undefined}
          size={32}
          className="mt-0.5 shrink-0"
        />
        <div className="min-w-0 flex-1">
          <div className="flex items-baseline gap-1.5">
            <span className="truncate text-[0.84375rem] font-semibold text-primary">{senderName}</span>
            {senderAddr && senderAddr !== senderName && (
              <span className="truncate text-[0.71875rem] text-secondary/80">{senderAddr}</span>
            )}
          </div>
          {(recipients || canOpen) && (
            <div className="flex min-w-0 items-center gap-1">
              {recipients && (
                <span
                  className="truncate text-[0.71875rem] text-secondary/80"
                  title={[tab.to, tab.cc].filter(Boolean).join(', ')}
                >
                  {t('chat.toRecipients', { recipients })}
                </span>
              )}
              {canOpen && (
                <button
                  type="button"
                  onClick={() => setOpen((value) => !value)}
                  title={open ? t('chat.hideDetails') : t('chat.showDetails')}
                  aria-expanded={open}
                  className="flex h-4 w-4 shrink-0 items-center justify-center rounded text-secondary hover:bg-black/[0.05] hover:text-primary dark:hover:bg-white/[0.08] cursor-pointer transition-colors select-none"
                >
                  <ChevronDown size={12} className={`transition-transform ${open ? 'rotate-180' : ''}`} />
                </button>
              )}
            </div>
          )}
        </div>
        <div className="flex shrink-0 items-center gap-1.5 text-[0.65625rem] text-secondary/80 select-none">
          {remote}
          {stamp && <span>{stamp}</span>}
        </div>
      </div>
      {open && (
        <div className="mt-2.5 space-y-1 border-t border-border/40 pt-2.5 text-secondary">
          {tab.fromRaw && <AddressRow label={t('composer.fields.from')} rawList={tab.fromRaw} />}
          {tab.to && <AddressRow label={t('composer.fields.to')} rawList={tab.to} />}
          {tab.cc && <AddressRow label={t('composer.fields.cc')} rawList={tab.cc} />}
          {tab.bcc && <AddressRow label={t('composer.fields.bcc')} rawList={tab.bcc} />}
          {replyToDiffers && <AddressRow label={t('chat.replyTo')} rawList={tab.replyTo!} />}
        </div>
      )}
    </div>
  )
}
