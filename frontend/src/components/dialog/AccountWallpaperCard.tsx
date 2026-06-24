import { useState } from 'react'
import { Image as ImageIcon } from 'lucide-react'
import { useTranslation } from '../../lib/i18n'
import type { Account } from '../../types'
import { WALLPAPER_PRESETS, sanitizeChatWallpaper, wallpaperCss } from '../../lib/wallpapers'
import { readFileData } from '../../lib/readFileData'
import { setAccountChatWallpaper, writeAccountChatWallpaperFile } from '../../states/accounts'
import { WallpaperDialog } from './WallpaperDialog'
import { SettingRow } from './AccountSettingsRows'

// Chat background row; renders inside the account panel's Appearance group.
export function AccountWallpaperCard({ account }: { account: Account }) {
  const { t } = useTranslation()
  const [dialogOpen, setDialogOpen] = useState(false)
  const wallpaper = sanitizeChatWallpaper(account.chat_wallpaper)

  const presetName =
    wallpaper?.kind === 'preset'
      ? (WALLPAPER_PRESETS.find((p) => p.id === wallpaper.presetId)?.name ?? t('wallpaper.plain'))
      : wallpaper?.kind === 'custom'
        ? t('wallpaper.customBackground')
        : t('wallpaper.plainDefault')

  const previewInfo = wallpaperCss(wallpaper)

  return (
    <SettingRow
      icon={<ImageIcon size={15} />}
      title={t('settings.account.chatBackground')}
      control={
        <div className="flex items-center gap-3 select-none">
          <span className="text-[11px] font-semibold text-secondary truncate max-w-32">{presetName}</span>
          <div
            className={`h-7 w-11 rounded-lg border border-border/80 overflow-hidden relative shadow-inner shrink-0 ${previewInfo.className}`}
            style={previewInfo.style}
          />
          <button
            type="button"
            onClick={() => setDialogOpen(true)}
            className="rounded-xl bg-active border border-border/30 px-3 py-1.5 text-xs font-semibold text-primary hover:bg-active hover:border-border/50 cursor-pointer transition-colors"
          >
            {t('common.change')}
          </button>
          {dialogOpen && (
            <WallpaperDialog
              title={t('wallpaper.chatTitle', { name: account.name || account.email })}
              previewName={account.name}
              wallpaper={account.chat_wallpaper}
              onSelect={(next) => setAccountChatWallpaper(account.id, next)}
              onUploadFile={async (file) => {
                const data = await readFileData(file)
                const url = await writeAccountChatWallpaperFile(account.id, {
                  name: file.name,
                  mime: file.type,
                  data,
                })
                await setAccountChatWallpaper(account.id, { kind: 'custom', url })
              }}
              onClose={() => setDialogOpen(false)}
            />
          )}
        </div>
      }
    />
  )
}
