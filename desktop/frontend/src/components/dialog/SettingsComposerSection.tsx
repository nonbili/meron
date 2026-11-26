import { useTranslation } from '../../lib/i18n'
import { Send, SpellCheck, Quote } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { settings$, sendShortcutLabel, type SendShortcut } from '../../states/settings'
import { SegmentedRow, SettingsGroup, ToggleRow } from './AccountSettingsRows'
import { SignatureSettingsSection } from './SignatureSettingsCard'

const SEND_SHORTCUT_OPTIONS: { value: SendShortcut; label: string }[] = [
  { value: 'enter', label: sendShortcutLabel('enter') },
  { value: 'mod_enter', label: sendShortcutLabel('mod_enter') },
]

export function SettingsComposerSection() {
  const { t } = useTranslation()
  const sendShortcut = useValue(settings$.sendShortcut)
  const spellCheck = useValue(settings$.spellCheck)
  const quoteReplies = useValue(settings$.quoteReplies)

  return (
    <div className="flex flex-col gap-4">
      <SettingsGroup title={t('settings.sections.general')}>
        <ToggleRow
          icon={<SpellCheck size={15} />}
          title={t('settings.composer.spellCheck')}
          hint={t('settings.composer.spellCheckHint')}
          checked={spellCheck}
          onChange={() => settings$.spellCheck.set(!spellCheck)}
        />
        <ToggleRow
          icon={<Quote size={15} />}
          title={t('settings.composer.quoteReplies')}
          hint={t('settings.composer.quoteRepliesHint')}
          checked={quoteReplies}
          onChange={() => settings$.quoteReplies.set(!quoteReplies)}
        />
        <SegmentedRow
          icon={<Send size={15} />}
          title={t('settings.composer.sendMessageWith')}
          hint={
            sendShortcut === 'enter'
              ? t('settings.composer.sendShortcutEnterHint')
              : t('settings.composer.sendShortcutModHint', { shortcut: sendShortcutLabel('mod_enter') })
          }
          value={sendShortcut}
          options={SEND_SHORTCUT_OPTIONS}
          onChange={(value) => settings$.sendShortcut.set(value)}
        />
      </SettingsGroup>

      <SignatureSettingsSection />
    </div>
  )
}
