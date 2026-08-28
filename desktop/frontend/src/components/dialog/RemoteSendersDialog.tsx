import { useState } from 'react'
import { Image as ImageIcon, Trash2, X } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from '../../lib/i18n'
import { useEscapeKey } from '../../lib/useEscapeKey'
import { settings$, setRemoteImageSender } from '../../states/settings'
import { IconButton } from '../button/IconButton'
import { TextInput } from '../field/Field'

/**
 * The app-wide remote-content allowlist: every sender the user trusted with
 * "Always allow from …" on a blocked message. Lives behind a count row in
 * Settings so a long list never stretches the settings card, and exists mainly
 * so a decision made in a hurry can be taken back.
 */
export function RemoteSendersDialog({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation()
  const senders = useValue(settings$.remoteImageSenders)
  const [filter, setFilter] = useState('')

  useEscapeKey(onClose, true)

  const query = filter.trim().toLowerCase()
  const visible = query ? senders.filter((sender) => sender.includes(query)) : senders

  return (
    <div className="fixed inset-0 z-[70] flex items-center justify-center bg-black/45 dark:bg-black/65 backdrop-blur-[3px] p-4 animate-fade-in">
      <div className="flex w-full max-w-md h-[520px] max-h-[85vh] flex-col overflow-hidden rounded-3xl border border-border bg-chats text-primary shadow-2xl animate-slide-up">
        <div className="flex shrink-0 items-center justify-between gap-3 border-b border-border/70 px-6 py-4">
          <div className="flex items-center gap-2">
            <ImageIcon className="text-accent" size={16} />
            <h3 className="text-[0.875rem] font-bold leading-tight">{t('settings.privacy.remoteSenders')}</h3>
          </div>
          <IconButton icon={X} iconSize={15} label={t('buttons.close')} size="sm" onClick={onClose} />
        </div>

        <div className="flex flex-1 flex-col gap-3 overflow-hidden px-6 py-4">
          <p className="text-[0.75rem] text-secondary">{t('settings.privacy.remoteSendersHint')}</p>
          {senders.length > 0 && (
            <TextInput
              type="search"
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
              placeholder={t('settings.privacy.filterSenders')}
              className="w-full"
            />
          )}
          <div className="flex-1 overflow-y-auto rounded-xl border border-border divide-y divide-border/40">
            {visible.map((sender) => (
              <div key={sender} className="flex items-center gap-2 px-3 py-2">
                <span className="flex-1 truncate text-xs font-semibold">{sender}</span>
                <button
                  onClick={() => setRemoteImageSender(sender, false)}
                  className="flex h-6 w-6 shrink-0 cursor-pointer items-center justify-center rounded-full text-secondary transition-colors hover:bg-active"
                  aria-label={t('settings.privacy.removeRemoteSender')}
                >
                  <Trash2 size={13} />
                </button>
              </div>
            ))}
            {visible.length === 0 && (
              <p className="px-3 py-6 text-center text-[0.75rem] text-secondary">
                {senders.length === 0 ? t('settings.privacy.noRemoteSenders') : t('settings.privacy.noMatchingSenders')}
              </p>
            )}
          </div>
        </div>
      </div>
    </div>
  )
}
