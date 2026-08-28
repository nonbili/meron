import { useState } from 'react'
import { ChevronDown, ChevronRight, Eye, EyeOff, Info, RefreshCw } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import { Field } from '../field/Field'
import { securityAfterPortEdit, type MailSecurity } from './accountSecurity'
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
  const [showPassword, setShowPassword] = useState(false)
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
    editing,
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
        // The address is the account's primary key in the store, so letting it
        // be edited here would create a second account rather than change this
        // one. Renaming is Remove + Add, deliberately.
        disabled={editing}
      />
      {(discovering || discoverNote) && (
        <p
          className={`flex items-center gap-1.5 px-1 -mt-2 text-[0.6875rem] font-medium ${discovering ? 'text-secondary' : discoverNote.startsWith("Couldn't") ? 'text-amber-600 dark:text-amber-400' : 'text-accent'}`}
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
      <label className="flex flex-col gap-1.5 w-full">
        <span className={`pl-0.5 ${fieldLabelClass ?? 'text-[0.6875rem] font-semibold text-secondary'}`}>
          {editing ? t('accounts.fields.passwordUnchanged') : t('accounts.fields.password')}
        </span>
        <span className="relative flex items-center">
          <input
            type={showPassword ? 'text' : 'password'}
            value={form.password}
            onChange={(event) => setForm((f) => ({ ...f, password: event.target.value }))}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && !saveDisabled) {
                e.preventDefault()
                void save()
              }
            }}
            className={`${inputClass ?? 'w-full text-xs py-2 px-3.5 rounded-xl border border-border bg-raised text-primary placeholder-secondary focus:ring-1 focus:ring-accent focus:border-transparent focus:bg-chats transition-all outline-none'} pr-11`}
          />
          <button
            type="button"
            onMouseDown={(event) => event.preventDefault()}
            onClick={() => setShowPassword((value) => !value)}
            className="absolute right-2.5 flex h-7 w-7 items-center justify-center rounded-lg text-secondary transition-colors hover:bg-hover hover:text-primary cursor-pointer"
            aria-label={
              showPassword
                ? t('accounts.actions.hidePassword', { defaultValue: 'Hide password' })
                : t('accounts.actions.showPassword', { defaultValue: 'Show password' })
            }
          >
            {showPassword ? <EyeOff size={15} /> : <Eye size={15} />}
          </button>
        </span>
      </label>

      {appPasswordHint && (
        <div
          className={`${isSetup ? 'rounded-2xl p-4 text-sm gap-3' : 'rounded-xl p-3 text-[0.6875rem] gap-2'} flex items-start bg-accent/[0.06] border border-accent/15 leading-relaxed text-secondary -mt-1`}
        >
          <Info size={isSetup ? 16 : 14} className="shrink-0 mt-0.5 text-accent" />
          <p className="flex-1 font-medium">{t('accounts.appPasswordHint', { provider: appPasswordHint.provider })}</p>
        </div>
      )}

      <button
        type="button"
        onClick={() => setAdvancedOpen((v) => !v)}
        className="flex items-center gap-1 self-start px-1 text-[0.6875rem] font-semibold text-secondary hover:text-primary transition-colors cursor-pointer"
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
              onChange={(imap_host) => setForm((f) => ({ ...f, imap_host, imap_host_touched: !!imap_host.trim() }))}
              inputClassName={inputClass}
              labelClassName={fieldLabelClass}
            />
            <Field
              label={t('accounts.fields.port')}
              value={form.imap_port}
              onChange={(imap_port) =>
                setForm((f) => ({
                  ...f,
                  imap_port,
                  imap_port_touched: !!imap_port.trim(),
                  imap_security: securityAfterPortEdit(f.imap_security, f.imap_security_touched, imap_port),
                }))
              }
              inputClassName={inputClass}
              labelClassName={fieldLabelClass}
            />
          </div>
          <SecurityField
            label={t('accounts.fields.imapSecurity')}
            value={form.imap_security}
            onChange={(imap_security) => setForm((f) => ({ ...f, imap_security, imap_security_touched: true }))}
            inputClassName={inputClass}
            labelClassName={fieldLabelClass}
            noneLabel={t('accounts.security.none')}
          />
          <div className={serverGridClass}>
            <Field
              label={t('accounts.fields.smtpHost')}
              value={form.smtp_host}
              onChange={(smtp_host) => setForm((f) => ({ ...f, smtp_host, smtp_host_touched: !!smtp_host.trim() }))}
              inputClassName={inputClass}
              labelClassName={fieldLabelClass}
            />
            <Field
              label={t('accounts.fields.port')}
              value={form.smtp_port}
              onChange={(smtp_port) =>
                setForm((f) => ({
                  ...f,
                  smtp_port,
                  smtp_port_touched: !!smtp_port.trim(),
                  smtp_security: securityAfterPortEdit(f.smtp_security, f.smtp_security_touched, smtp_port),
                }))
              }
              inputClassName={inputClass}
              labelClassName={fieldLabelClass}
            />
          </div>
          <SecurityField
            label={t('accounts.fields.smtpSecurity')}
            value={form.smtp_security}
            onChange={(smtp_security) => setForm((f) => ({ ...f, smtp_security, smtp_security_touched: true }))}
            inputClassName={inputClass}
            labelClassName={fieldLabelClass}
            noneLabel={t('accounts.security.none')}
          />
          <Field
            label={t('accounts.fields.username')}
            value={form.username}
            onChange={(username) => setForm((f) => ({ ...f, username, username_touched: !!username.trim() }))}
            inputClassName={inputClass}
            labelClassName={fieldLabelClass}
          />
        </>
      )}
    </>
  )
}

function SecurityField({
  label,
  value,
  onChange,
  inputClassName,
  labelClassName,
  noneLabel,
}: {
  label: string
  value: MailSecurity
  onChange: (value: MailSecurity) => void
  inputClassName?: string
  labelClassName?: string
  noneLabel: string
}) {
  return (
    <label className="flex flex-col gap-1.5 w-full">
      <span className={`pl-0.5 ${labelClassName ?? 'text-[0.6875rem] font-semibold text-secondary'}`}>{label}</span>
      <span className="relative flex items-center">
        <select
          value={value}
          onChange={(event) => onChange(event.target.value as MailSecurity)}
          className={`${
            inputClassName ??
            'w-full text-xs py-2 px-3.5 rounded-xl border border-border bg-raised text-primary focus:ring-1 focus:ring-accent focus:border-transparent focus:bg-chats transition-all outline-none'
          } appearance-none pr-10 cursor-pointer`}
        >
          <option value="tls">TLS</option>
          <option value="starttls">STARTTLS</option>
          <option value="none">{noneLabel}</option>
        </select>
        <ChevronDown size={14} aria-hidden="true" className="pointer-events-none absolute right-3 text-secondary" />
      </span>
    </label>
  )
}
