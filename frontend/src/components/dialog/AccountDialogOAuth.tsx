import type { AccountDialogController } from './useAccountDialog'
import { GoogleIcon, MicrosoftIcon } from './providerIcons'
import { useTranslation } from '../../lib/i18n'

// The OAuth sign-in section mirrors mobile: one row per supported OAuth
// provider. The outer provider rail stays generic so more providers can be added
// here later without adding more account-type tabs.
export function AccountDialogOAuth({ ctl, isSetup }: { ctl: AccountDialogController; isSetup: boolean }) {
  const { t } = useTranslation()
  const { mode, oauthLabel, gmailConfigured, outlookConfigured, waitingForGoogle, beginOAuth } = ctl
  const providerButtons = [
    {
      mode: 'gmail' as const,
      label: t('accounts.oauth.signInWith', { provider: 'Google' }),
      configured: gmailConfigured,
      icon: <GoogleIcon size={isSetup ? 20 : 16} />,
    },
    {
      mode: 'outlook' as const,
      label: t('accounts.oauth.signInWith', { provider: 'Outlook' }),
      configured: outlookConfigured,
      icon: <MicrosoftIcon size={isSetup ? 20 : 16} />,
    },
  ]

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-col gap-4">
        {providerButtons.map((provider) => {
          const busy = waitingForGoogle && mode === provider.mode
          return (
            <div key={provider.mode} className="flex flex-col gap-1.5">
              <button
                type="button"
                disabled={waitingForGoogle || !provider.configured}
                onClick={() => void beginOAuth(provider.mode)}
                className={`w-full flex items-center justify-center border bg-chats font-semibold transition-all ${
                  isSetup
                    ? 'gap-3 rounded-2xl border-border px-5 py-4 text-[18px] text-primary hover:border-secondary/50 hover:bg-hover'
                    : 'gap-2 rounded-xl border-border px-4 py-2.5 text-xs shadow-sm hover:bg-hover'
                } ${busy ? 'opacity-70 cursor-wait' : ''} ${
                  !provider.configured ? 'opacity-60 cursor-not-allowed' : 'cursor-pointer'
                }`}
              >
                {provider.icon}
                {provider.label}
              </button>
            </div>
          )
        })}
      </div>
      {waitingForGoogle && (
        <p className={`${isSetup ? 'text-base' : 'text-[10px]'} text-accent font-semibold text-center animate-pulse`}>
          {t('accounts.oauth.waitingForSignIn', { provider: oauthLabel })}
        </p>
      )}
    </div>
  )
}
