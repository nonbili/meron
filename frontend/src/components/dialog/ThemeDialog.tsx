import { useEffect, useState } from 'react'
import { Palette, Plus, X } from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { useTranslation } from 'react-i18next'
import { BUILTIN_THEMES, DEFAULT_LIGHT_ID, type Appearance, type CustomTheme, type ThemeDef } from '../../lib/themes'
import { confirmAction } from '../../states/ui'
import { deleteCustomTheme, selectTheme, settings$ } from '../../states/settings'
import { IconButton } from '../button/IconButton'
import { ThemeEditorDialog } from './ThemeEditorDialog'
import { ThemeSwatch } from './ThemeSwatch'

// Theme picker dialog (Settings -> General -> Theme -> Change), following the
// WallpaperDialog layout: light and dark sections of large swatches, with the
// custom-theme editor reachable from a dashed tile in each section.

type EditorState = { appearance: Appearance; theme: CustomTheme | null }

function ThemeSection({
  label,
  themes,
  customThemes,
  effectiveId,
  onEdit,
  onDelete,
  newTileAppearance,
}: {
  label: string
  themes: ThemeDef[]
  customThemes: CustomTheme[]
  effectiveId: string
  onEdit: (state: EditorState) => void
  onDelete: (theme: CustomTheme) => void
  newTileAppearance: Appearance
}) {
  const { t } = useTranslation()
  return (
    <div className="flex flex-col gap-3">
      <span className="text-[12.5px] font-semibold text-secondary">{label}</span>
      <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 gap-3">
        {themes.map((item) => {
          const custom = customThemes.find((candidate) => candidate.id === item.id)
          return (
            <ThemeSwatch
              key={item.id}
              theme={item}
              large
              selected={item.id === effectiveId}
              onSelect={() => selectTheme(item)}
              onEdit={custom ? () => onEdit({ appearance: custom.appearance, theme: custom }) : undefined}
              onDelete={custom ? () => onDelete(custom) : undefined}
            />
          )
        })}
        <button
          type="button"
          onClick={() => onEdit({ appearance: newTileAppearance, theme: null })}
          className="flex min-h-[112px] flex-col items-center justify-center gap-1.5 rounded-xl border border-dashed border-border text-secondary hover:text-accent hover:border-accent/50 cursor-pointer transition-colors"
        >
          <Plus size={16} />
          <span className="text-[10.5px] font-bold">{t('theme.custom')}</span>
        </button>
      </div>
    </div>
  )
}

export function ThemeDialog({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation()
  const [editor, setEditor] = useState<EditorState | null>(null)
  const selectedId = useValue(settings$.themeId)
  const customThemes = useValue(settings$.customThemes)

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.stopPropagation()
        onClose()
      }
    }
    window.addEventListener('keydown', onKeyDown, true)
    return () => window.removeEventListener('keydown', onKeyDown, true)
  }, [onClose])

  const themes = [...BUILTIN_THEMES, ...customThemes]
  // A stale selection (deleted custom theme) highlights the default, matching
  // what resolveThemeDef actually paints.
  const effectiveId = themes.some((item) => item.id === selectedId) ? selectedId : DEFAULT_LIGHT_ID

  const onDelete = async (themeToDelete: CustomTheme) => {
    const confirmed = await confirmAction({
      title: t('theme.delete'),
      message: t('theme.deleteMessage', { name: themeToDelete.name }),
      confirmLabel: t('buttons.delete'),
      tone: 'danger',
    })
    if (confirmed) deleteCustomTheme(themeToDelete.id)
  }

  const sectionProps = {
    customThemes,
    effectiveId,
    onEdit: (state: EditorState) => setEditor(state),
    onDelete: (theme: CustomTheme) => void onDelete(theme),
  }

  return (
    <>
      <div className="fixed inset-0 z-[70] flex items-center justify-center bg-black/45 dark:bg-black/65 backdrop-blur-[3px] p-4 animate-fade-in">
        <div className="w-full max-w-3xl h-[620px] max-h-[90vh] rounded-3xl border border-border bg-chats text-primary shadow-2xl animate-slide-up flex flex-col overflow-hidden">
          <div className="flex items-center justify-between gap-3 border-b border-border/70 px-6 py-4 shrink-0">
            <div className="flex items-center gap-2">
              <Palette className="text-accent" size={16} />
              <h3 className="text-[14px] font-bold leading-tight">{t('common.theme')}</h3>
            </div>
            <IconButton icon={X} iconSize={15} label={t('buttons.close')} size="sm" onClick={onClose} />
          </div>

          <div className="flex-1 overflow-y-auto p-6 flex flex-col gap-6">
            <ThemeSection
              label={t('theme.light')}
              themes={themes.filter((item) => item.appearance === 'light')}
              newTileAppearance="light"
              {...sectionProps}
            />
            <ThemeSection
              label={t('theme.dark')}
              themes={themes.filter((item) => item.appearance === 'dark')}
              newTileAppearance="dark"
              {...sectionProps}
            />
          </div>
        </div>
      </div>

      {editor && (
        <ThemeEditorDialog appearance={editor.appearance} initial={editor.theme} onClose={() => setEditor(null)} />
      )}
    </>
  )
}
