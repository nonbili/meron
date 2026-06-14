import { ChevronRight, RefreshCw } from 'lucide-react'
import { Trans, useTranslation } from 'react-i18next'
import { openExternal } from '../../lib/native'
import { Field } from '../field/Field'
import type { AccountDialogController } from './useAccountDialog'
import type { DialogClasses } from './accountDialogStyles'

// The custom IMAP/SMTP section: email (with autodiscovery), sender name, password,
// the app-password hint, and the collapsible advanced server settings.
export function AccountDialogCustom({
  ctl,
  classes,
  isSetup,
}: {
  ctl: AccountDialogController
  classes: DialogClasses
  isSetup: boolean
}) {
  const { t } = useTranslation()
  const {
    form,
    setForm,
    discovering,
    discoverNote,
    appPasswordHint,
    advancedOpen,
    setAdvancedOpen,
    runDiscovery,
    save,
    saveDisabled,
  } = ctl
  const { inputClass, fieldLabelClass, serverGridClass } = classes

  return (
    <>
      <Field
        label={t('accounts.fields.emailAddress')}
        value={form.email}
        onChange={(email) => setForm((f) => ({ ...f, email }))}
        onBlur={() => runDiscovery(form.email)}
        inputClassName={inputClass}
        labelClassName={fieldLabelClass}
      />
      {(discovering || discoverNote) && (
        <p
          className={`flex items-center gap-1.5 px-1 -mt-2 text-[11px] font-medium ${discovering ? 'text-secondary' : discoverNote.startsWith("Couldn't") ? 'text-amber-600 dark:text-amber-400' : 'text-accent'}`}
        >
          {discovering && <RefreshCw size={11} className="animate-spin" />}
          {discovering ? t('accounts.discovery.lookingUp') : discoverNote}
        </p>
      )}
      <Field
        label={t('accounts.fields.senderNameOutgoing')}
        value={form.sender_name}
        onChange={(sender_name) => setForm((f) => ({ ...f, sender_name }))}
        inputClassName={inputClass}
        labelClassName={fieldLabelClass}
      />
      <Field
        label={t('accounts.fields.password')}
        type="password"
        value={form.password}
        onChange={(password) => setForm((f) => ({ ...f, password }))}
        onKeyDown={(e) => {
          if (e.key === 'Enter' && !saveDisabled) {
            e.preventDefault()
            void save()
          }
        }}
        inputClassName={inputClass}
        labelClassName={fieldLabelClass}
      />

      {appPasswordHint && (
        <p
          className={`${isSetup ? 'rounded-2xl p-4 text-sm' : 'rounded-xl p-3 text-[11px]'} bg-amber-50 dark:bg-amber-950/20 border border-amber-200 dark:border-amber-900/50 leading-relaxed text-amber-700 dark:text-amber-300 font-medium -mt-1`}
        >
          <Trans
            i18nKey="accounts.appPasswordHint"
            values={{ provider: appPasswordHint.provider }}
            components={{
              action: (
                <button
                  type="button"
                  onClick={() => openExternal(appPasswordHint.url)}
                  className="underline underline-offset-2 hover:text-amber-900 dark:hover:text-amber-100 cursor-pointer font-semibold"
                />
              ),
            }}
          />
        </p>
      )}

      <button
        type="button"
        onClick={() => setAdvancedOpen((v) => !v)}
        className="flex items-center gap-1 self-start px-1 text-[11px] font-semibold text-secondary hover:text-primary transition-colors cursor-pointer"
      >
        <ChevronRight size={12} className={`transition-transform ${advancedOpen ? 'rotate-90' : ''}`} />
        {t('accounts.advancedServerSettings')}
      </button>

      {advancedOpen && (
        <>
          <Field
            label={t('accounts.fields.displayNameMeronOnly')}
            value={form.display_name}
            onChange={(display_name) => setForm((f) => ({ ...f, display_name }))}
            inputClassName={inputClass}
            labelClassName={fieldLabelClass}
          />
          <div className={serverGridClass}>
            <Field
              label={t('accounts.fields.imapHost')}
              value={form.imap_host}
              onChange={(imap_host) => setForm((f) => ({ ...f, imap_host }))}
              inputClassName={inputClass}
              labelClassName={fieldLabelClass}
            />
            <Field
              label={t('accounts.fields.port')}
              value={form.imap_port}
              onChange={(imap_port) => setForm((f) => ({ ...f, imap_port }))}
              inputClassName={inputClass}
              labelClassName={fieldLabelClass}
            />
          </div>
          <div className={serverGridClass}>
            <Field
              label={t('accounts.fields.smtpHost')}
              value={form.smtp_host}
              onChange={(smtp_host) => setForm((f) => ({ ...f, smtp_host }))}
              inputClassName={inputClass}
              labelClassName={fieldLabelClass}
            />
            <Field
              label={t('accounts.fields.port')}
              value={form.smtp_port}
              onChange={(smtp_port) => setForm((f) => ({ ...f, smtp_port }))}
              inputClassName={inputClass}
              labelClassName={fieldLabelClass}
            />
          </div>
          <Field
            label={t('accounts.fields.username')}
            value={form.username}
            onChange={(username) => setForm((f) => ({ ...f, username }))}
            inputClassName={inputClass}
            labelClassName={fieldLabelClass}
          />
        </>
      )}
    </>
  )
}
