import type { AccountDialogController } from './useAccountDialog'
import { GoogleIcon, MicrosoftIcon } from './providerIcons'
import { useTranslation } from 'react-i18next'

// The Gmail/Outlook OAuth sign-in section: a provider sign-in button (with the
// provider glyph) plus the "configure client id" notice when OAuth isn't set up.
export function AccountDialogOAuth({ ctl, isSetup }: { ctl: AccountDialogController; isSetup: boolean }) {
  const { t } = useTranslation()
  const { mode, oauthConfigured, oauthLabel, waitingForGoogle, beginOAuth } = ctl

  if (!oauthConfigured) {
    return (
      <p
        className={`${isSetup ? 'rounded-2xl p-4 text-sm' : 'rounded-xl p-3 text-[10px]'} bg-amber-50 dark:bg-amber-950/20 border border-amber-200 dark:border-amber-900/50 leading-relaxed text-amber-600 dark:text-amber-400 font-medium`}
      >
        {mode === 'outlook' ? t('accounts.oauth.configureOutlook') : t('accounts.oauth.configureGmail')}
      </p>
    )
  }

  return (
    <div className="flex flex-col gap-2">
      <button
        type="button"
        disabled={waitingForGoogle}
        onClick={() => void beginOAuth()}
        className={`w-full flex items-center justify-center border bg-chats font-semibold transition-all ${
          isSetup
            ? 'gap-3 rounded-2xl border-border px-5 py-4 text-[18px] text-primary hover:border-secondary/50 hover:bg-hover'
            : 'gap-2 rounded-xl border-border px-4 py-2.5 text-xs shadow-sm hover:bg-hover'
        } ${waitingForGoogle ? 'opacity-70 cursor-wait' : ''}`}
      >
        {mode === 'outlook' ? <MicrosoftIcon size={isSetup ? 20 : 16} /> : <GoogleIcon size={isSetup ? 20 : 16} />}
        {t('accounts.oauth.signInWith', { provider: oauthLabel })}
      </button>
      {waitingForGoogle && (
        <p className={`${isSetup ? 'text-base' : 'text-[10px]'} text-accent font-semibold text-center animate-pulse`}>
          {t('accounts.oauth.waitingForSignIn', { provider: oauthLabel })}
        </p>
      )}
    </div>
  )
}
