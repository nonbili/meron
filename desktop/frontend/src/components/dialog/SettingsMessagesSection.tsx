import { useState } from 'react'
import { useTranslation } from '../../lib/i18n'
import { UnfoldVertical, KeyRound, MessagesSquare, Moon, Shrink, PanelBottom } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { settings$, type ConversationFooter, type ConversationLayout } from '../../states/settings'
import { SegmentedRow, SettingRow, SettingsGroup, ToggleRow } from './AccountSettingsRows'
import { FontSettingsSection } from './FontSettingsSection'
import { ConversationToolbarSettings } from './ConversationToolbarSettings'
import { TagsSection } from './TagsSection'
import { RemoteSendersDialog } from './RemoteSendersDialog'

const CONVERSATION_LAYOUT_OPTIONS = (
  t: ReturnType<typeof useTranslation>['t'],
): { value: ConversationLayout; label: string }[] => [
  { value: 'chat', label: t('settings.appearance.conversationLayoutChat') },
  { value: 'traditional', label: t('settings.appearance.conversationLayoutTraditional') },
]

const CONVERSATION_FOOTER_OPTIONS = (
  t: ReturnType<typeof useTranslation>['t'],
): { value: ConversationFooter; label: string }[] => [
  { value: 'reply', label: t('settings.appearance.conversationFooterReply') },
  { value: 'actions', label: t('settings.appearance.conversationFooterActions') },
  { value: 'none', label: t('settings.appearance.conversationFooterNone') },
]

// How received mail is shown and handled: layout, rendering, the reader's
// actions, message typography, and whose remote content loads.
export function SettingsMessagesSection() {
  const { t } = useTranslation()
  const [remoteSendersOpen, setRemoteSendersOpen] = useState(false)
  const remoteImageSenders = useValue(settings$.remoteImageSenders)
  const conversationLayout = useValue(settings$.conversationLayout)
  const chatFullMessages = useValue(settings$.chatFullMessages)
  const darkMessageBodies = useValue(settings$.darkMessageBodies)
  const autoFitMessages = useValue(settings$.autoFitMessages)
  const conversationFooter = useValue(settings$.conversationFooter)
  const detectOneTimeCodes = useValue(settings$.detectOneTimeCodes)

  return (
    <div className="flex flex-col gap-4">
      <SettingsGroup title={t('settings.pages.appearance')}>
        <SegmentedRow
          icon={<MessagesSquare size={15} />}
          title={t('settings.appearance.conversationLayout')}
          hint={t('settings.appearance.conversationLayoutHint')}
          value={conversationLayout}
          options={CONVERSATION_LAYOUT_OPTIONS(t)}
          onChange={(value) => settings$.conversationLayout.set(value)}
        />
        {conversationLayout === 'chat' && (
          <ToggleRow
            icon={<UnfoldVertical size={15} />}
            title={t('settings.appearance.chatFullMessages')}
            hint={t('settings.appearance.chatFullMessagesHint')}
            checked={chatFullMessages}
            onChange={() => settings$.chatFullMessages.set(!chatFullMessages)}
          />
        )}
        <ToggleRow
          icon={<Moon size={15} />}
          title={t('settings.appearance.darkMessageBodies')}
          hint={t('settings.appearance.darkMessageBodiesHint')}
          checked={darkMessageBodies}
          onChange={() => settings$.darkMessageBodies.set(!darkMessageBodies)}
        />
        <ToggleRow
          icon={<Shrink size={15} />}
          title={t('settings.appearance.autoFitMessages')}
          hint={t('settings.appearance.autoFitMessagesHint')}
          checked={autoFitMessages}
          onChange={() => settings$.autoFitMessages.set(!autoFitMessages)}
        />
        <SegmentedRow
          icon={<PanelBottom size={15} />}
          title={t('settings.appearance.conversationFooter')}
          hint={t('settings.appearance.conversationFooterHint')}
          value={conversationFooter}
          options={CONVERSATION_FOOTER_OPTIONS(t)}
          onChange={(value) => settings$.conversationFooter.set(value)}
        />
      </SettingsGroup>

      <ConversationToolbarSettings />

      <SettingsGroup title={t('settings.sections.typography')}>
        <FontSettingsSection part="message" />
      </SettingsGroup>

      <TagsSection />

      <SettingsGroup title={t('settings.sections.privacy')}>
        <SettingRow
          title={t('settings.privacy.remoteSenders')}
          hint={t('settings.privacy.remoteSendersHint')}
          control={
            <button
              onClick={() => setRemoteSendersOpen(true)}
              className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl bg-hover hover:bg-active text-primary font-bold text-[0.625rem] cursor-pointer transition-colors"
            >
              {remoteImageSenders.length > 0
                ? t('settings.privacy.remoteSendersCount', { count: remoteImageSenders.length })
                : t('settings.privacy.remoteSendersNone')}
            </button>
          }
        />
        <ToggleRow
          icon={<KeyRound size={15} />}
          title={t('settings.privacy.detectCodes')}
          hint={t('settings.privacy.detectCodesHint')}
          checked={detectOneTimeCodes}
          onChange={() => settings$.detectOneTimeCodes.set(!detectOneTimeCodes)}
        />
      </SettingsGroup>
      {remoteSendersOpen && <RemoteSendersDialog onClose={() => setRemoteSendersOpen(false)} />}
    </div>
  )
}
