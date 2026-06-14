import { useEffect } from 'react'
import { X, RefreshCw } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { ui$ } from '../../states/ui'
import { Field } from '../field/Field'
import { Button } from '../button/Button'
import { IconButton } from '../button/IconButton'
import logo from '../../assets/logo.png'
import { useAccountDialog, type AccountDialogController } from './useAccountDialog'
import { dialogClasses, type DialogClasses } from './accountDialogStyles'
import { AccountDialogOAuth } from './AccountDialogOAuth'
import { AccountDialogCustom } from './AccountDialogCustom'
import { AccountProviderGrid } from './AccountProviderGrid'
import { AccountProviderRail } from './AccountProviderRail'
import { PROVIDERS } from './providerIcons'

type AccountDialogProps = {
  variant?: 'dialog' | 'setup'
}

export function AccountDialog({ variant = 'dialog' }: AccountDialogProps) {
  const { t } = useTranslation()
  const isSetup = variant === 'setup'
  const ctl = useAccountDialog()
  const { mode, setMode } = ctl
  const classes = dialogClasses(isSetup)

  // OAuth providers sign in *and* save in one step (pollProfile saves the account
  // and closes the dialog on success), so the standalone "Save Account" button is
  // never reachable for them — the in-form sign-in button is the only CTA. Manual
  // setups (IMAP, RSS) still need the explicit Save button.
  const isOAuth = mode === 'gmail' || mode === 'outlook'

  const onClose = () => {
    if (isSetup) return
    ui$.setupOpen.set(false)
  }

  // Esc closes the layered add-account dialog. The full-screen onboarding setup
  // variant has no close affordance, so it ignores Esc.
  useEffect(() => {
    if (isSetup) return
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault()
        event.stopPropagation()
        onClose()
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [isSetup])

  // The full-screen first-run onboarding: provider cards stacked above the form.
  if (isSetup) {
    return (
      <div className={classes.panelClass}>
        <div>
          <div className="w-16 h-16 mb-4">
            <img src={logo} alt={t('app.logoAlt')} className="w-full h-full object-contain" />
          </div>
          <h2 className="text-[26px] max-[640px]:text-2xl font-bold tracking-tight leading-tight">
            {t('accounts.setup.connectMailAccount')}
          </h2>
          <p className="mt-2 text-[15px] leading-6 text-secondary">{t('accounts.setup.chooseProvider')}</p>
        </div>

        <AccountProviderGrid mode={mode} setMode={setMode} isSetup />

        <div className={classes.scrollClass}>
          <AccountDialogForm ctl={ctl} classes={classes} isSetup />
        </div>

        <AccountDialogError error={ctl.error} />

        {!isOAuth && (
          <div className="flex gap-2 mt-1 select-none justify-stretch">
            <SaveButton ctl={ctl} isSetup />
          </div>
        )}
      </div>
    )
  }

  // The in-app add-account modal: a provider rail on the left, the form for the
  // selected provider on the right — same shape as the Settings dialog.
  const active = PROVIDERS.find((p) => p.id === mode)
  return (
    <div className="fixed inset-0 flex items-center justify-center bg-black/40 dark:bg-black/60 backdrop-blur-[3px] z-50 p-4 select-none animate-fade-in">
      <div className="bg-chats border border-border text-primary max-w-xl w-full max-h-[90vh] rounded-3xl shadow-2xl animate-slide-up flex flex-col overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between gap-4 px-6 py-4 border-b border-border/70 shrink-0">
          <h2 className="text-[15px] font-bold tracking-tight leading-tight">
            {t('accounts.actions.addAccountTitle')}
          </h2>
          <IconButton icon={X} iconSize={15} label={t('buttons.close')} size="sm" onClick={onClose} />
        </div>

        {/* Body: provider rail + form. Fixed height so swapping providers (whose
            forms differ in length) doesn't make the dialog jump; the form column
            scrolls internally instead. */}
        <div className="flex h-[360px]">
          <AccountProviderRail mode={mode} setMode={setMode} />
          <div className="flex-1 min-w-0 overflow-y-auto p-5 flex flex-col gap-4">
            <div>
              <h3 className="text-[13px] font-bold tracking-tight leading-tight">{active?.label}</h3>
              <p className="text-[10.5px] text-secondary mt-0.5 font-medium">
                {active ? t(active.descriptionKey) : null}
              </p>
            </div>
            <div className="flex flex-col gap-3.5">
              <AccountDialogForm ctl={ctl} classes={classes} isSetup={false} />
            </div>
            <AccountDialogError error={ctl.error} />
          </div>
        </div>

        {/* Footer */}
        <div className="flex justify-end gap-2 px-6 py-4 border-t border-border/70 shrink-0 select-none">
          <Button variant="ghost" onClick={onClose}>
            {t('buttons.cancel')}
          </Button>
          {!isOAuth && <SaveButton ctl={ctl} isSetup={false} />}
        </div>
      </div>
    </div>
  )
}

// The mode-specific form fields, shared by both layouts.
function AccountDialogForm({
  ctl,
  classes,
  isSetup,
}: {
  ctl: AccountDialogController
  classes: DialogClasses
  isSetup: boolean
}) {
  const { t } = useTranslation()
  const { mode, form, setForm } = ctl
  if (mode === 'rss') {
    return (
      <>
        <Field
          label={t('accounts.fields.accountName')}
          value={form.display_name}
          onChange={(display_name) => setForm((f) => ({ ...f, display_name }))}
          inputClassName={classes.inputClass}
          labelClassName={classes.fieldLabelClass}
        />
        <Field
          label={t('accounts.fields.firstFeedUrl')}
          value={form.feed_url}
          onChange={(feed_url) => setForm((f) => ({ ...f, feed_url }))}
          inputClassName={classes.inputClass}
          labelClassName={classes.fieldLabelClass}
        />
        <p className="text-[11px] text-secondary px-1 -mt-1">{t('accounts.setup.feedAccountHint')}</p>
      </>
    )
  }
  if (mode === 'gmail' || mode === 'outlook') {
    return <AccountDialogOAuth ctl={ctl} isSetup={isSetup} />
  }
  return <AccountDialogCustom ctl={ctl} classes={classes} isSetup={isSetup} />
}

function AccountDialogError({ error }: { error: string }) {
  if (!error) return null
  return (
    <p className="rounded-xl bg-red-50 dark:bg-red-950/20 border border-red-200 dark:border-red-900/50 p-3 text-[11px] leading-relaxed text-red-600 dark:text-red-400 font-medium">
      {error}
    </p>
  )
}

function SaveButton({ ctl, isSetup }: { ctl: AccountDialogController; isSetup: boolean }) {
  const { t } = useTranslation()
  const { save, saveDisabled, loading } = ctl
  return (
    <button
      onClick={save}
      disabled={saveDisabled}
      className={`${isSetup ? 'w-full rounded-2xl py-4 text-[18px]' : 'rounded-xl px-4.5 py-2 text-xs'} font-bold transition-all flex items-center justify-center gap-1.5 cursor-pointer ${
        saveDisabled
          ? isSetup
            ? 'bg-hover text-secondary/70 cursor-not-allowed border border-transparent shadow-none'
            : 'bg-hover text-secondary/70 cursor-not-allowed shadow-none border border-transparent'
          : isSetup
            ? 'border border-accent bg-accent text-white hover:bg-accent-hover hover:border-accent-hover active:scale-[0.99] shadow-md shadow-accent/15'
            : 'bg-accent hover:bg-accent-hover text-white shadow-md shadow-accent/15 hover:shadow-lg hover:shadow-accent/20 active:scale-98'
      }`}
    >
      {loading && <RefreshCw size={11} className="animate-spin" />}
      <span>{t('accounts.actions.saveAccount')}</span>
    </button>
  )
}
