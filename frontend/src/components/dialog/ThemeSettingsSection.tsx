import { useState } from 'react'
import { Palette } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useValue } from '@legendapp/state/react'
import { BUILTIN_THEMES, DEFAULT_LIGHT_ID } from '../../lib/themes'
import { settings$ } from '../../states/settings'
import { ThemeDialog } from './ThemeDialog'
import { SettingRow } from './AccountSettingsRows'

// The Theme row of Settings -> General: shows the active theme with a mini
// preview; picking happens in ThemeDialog (mirrors AccountWallpaperCard).

export function ThemeSettingsSection() {
  const { t: translate } = useTranslation()
  const [dialogOpen, setDialogOpen] = useState(false)
  const selectedId = useValue(settings$.themeId)
  const customThemes = useValue(settings$.customThemes)

  const themes = [...BUILTIN_THEMES, ...customThemes]
  // A stale selection (deleted custom theme) shows the default, matching what
  // resolveThemeDef actually paints.
  const active = themes.find((item) => item.id === selectedId) ?? themes.find((item) => item.id === DEFAULT_LIGHT_ID)!
  const t = active.tokens

  return (
    <SettingRow
      icon={<Palette size={15} />}
      title={translate('common.theme')}
      control={
        <div className="flex items-center gap-3 select-none">
          <span className="text-[11px] font-semibold text-secondary truncate max-w-32">{active.name}</span>
          <div
            className="h-7 w-11 rounded-lg border border-border/80 overflow-hidden relative shadow-inner shrink-0 flex"
            style={{ background: t.bgApp }}
          >
            <span className="w-1 shrink-0" style={{ background: t.bgSidebar }} />
            <span className="w-3 shrink-0 border-r" style={{ background: t.bgChats, borderColor: t.border }} />
            <span className="flex min-w-0 flex-1 flex-col justify-center gap-0.5 px-1">
              <span
                className="h-1 w-3 self-start rounded-full"
                style={{ background: t.bubbleIn, boxShadow: `inset 0 0 0 1px ${t.border}` }}
              />
              <span className="h-1 w-3 self-end rounded-full" style={{ background: t.bubbleOut }} />
              <span className="h-1 w-2 self-start rounded-full" style={{ background: t.accent }} />
            </span>
          </div>
          <button
            type="button"
            onClick={() => setDialogOpen(true)}
            className="rounded-xl bg-active border border-border/30 px-3 py-1.5 text-xs font-semibold text-primary hover:bg-active hover:border-border/50 cursor-pointer transition-colors"
          >
            {translate('common.change')}
          </button>
          {dialogOpen && <ThemeDialog onClose={() => setDialogOpen(false)} />}
        </div>
      }
    />
  )
}
