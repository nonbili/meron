import { useTranslation } from '../../lib/i18n'
import {
  CalendarDays,
  CircleUser,
  Image as ImageIcon,
  Inbox,
  Columns3,
  Lock,
  Globe,
  Bell,
  RefreshCw,
  Keyboard,
  PanelTop,
  PanelTopClose,
  Contrast,
  AppWindow,
  Rows3,
} from 'lucide-react'
import { useValue } from '@legendapp/state/react'
import { showToast, ui$ } from '../../states/ui'
import {
  settings$,
  appIconSupported$,
  appIconPlatform$,
  clampKanbanColumnWidth,
  KANBAN_COLUMN_MAX_WIDTH,
  KANBAN_COLUMN_MIN_WIDTH,
  setUnifiedInboxSideNavVisible,
  setCalendarSideNavVisible,
} from '../../states/settings'
import { update$ } from '../../states/update'
import { NumberRow, SettingRow, SettingsGroup, ToggleRow, SelectRow } from './AccountSettingsRows'
import { supportedI18nLanguages, languageNativeNames, type SupportedI18nLanguage } from '../../lib/i18n'
import { ThemeSettingsSection } from './ThemeSettingsSection'
import { FontSettingsSection } from './FontSettingsSection'
import { ProxySettingsSection } from './ProxySettingsCard'
import { ColorSwatchPicker } from '../field/ColorSwatchPicker'
import { isMac } from '../../lib/shortcuts'
import { offersMonochromeTrayIcon } from '../../lib/monochromeTrayIcon'
import { isWindowsPlatform } from '../../lib/platform'
import { appIconHintKey, parseAppIconColour } from '../../lib/appIconStyle'
import { setIntegratedTitlebar, windowChrome$ } from '../../lib/windowChrome'
import { SettingsBackupRow, SettingsStorageRows, SettingsLogsRow } from './SettingsMaintenanceRows'

export function SettingsGeneralSection() {
  const { t } = useTranslation()
  const showRealAvatars = useValue(settings$.showRealAvatars)
  const showThreadAvatars = useValue(settings$.showThreadAvatars)
  const threadPreviewLine = useValue(settings$.threadPreviewLine)
  const showUnreadAccountBadge = useValue(settings$.showUnreadAccountBadge)
  const showUnifiedInbox = useValue(settings$.showUnifiedInboxInSideNav)
  const showCalendarInSideNav = useValue(settings$.showCalendarInSideNav)
  const calendarEnabled = useValue(settings$.calendarEnabled)
  const kanbanColumnWidth = useValue(settings$.kanbanColumnWidth)
  const kanbanLockScroll = useValue(settings$.kanbanLockScroll)
  const language = useValue(settings$.language)

  return (
    <div className="flex flex-col gap-4">
      <SettingsGroup title={t('settings.pages.appearance')}>
        <ThemeSettingsSection />
        <SelectRow
          icon={<Globe size={15} />}
          title={t('settings.language.label')}
          hint={t('settings.language.hint')}
          value={language || ''}
          options={[
            { value: '', label: t('settings.language.system') },
            ...supportedI18nLanguages.map((lang) => ({
              value: lang,
              label: languageNativeNames[lang],
            })),
          ]}
          onChange={(value) => {
            settings$.language.set(value === '' ? null : (value as SupportedI18nLanguage))
          }}
        />
        <ToggleRow
          icon={<ImageIcon size={15} />}
          title={t('settings.appearance.showSenderImages')}
          hint={t('settings.appearance.showSenderImagesHint')}
          checked={showRealAvatars}
          onChange={() => settings$.showRealAvatars.set(!showRealAvatars)}
        />
        <ToggleRow
          icon={<CircleUser size={15} />}
          title={t('settings.appearance.showThreadAvatars')}
          hint={t('settings.appearance.showThreadAvatarsHint')}
          checked={showThreadAvatars}
          onChange={() => settings$.showThreadAvatars.set(!showThreadAvatars)}
        />
        <ToggleRow
          icon={<Rows3 size={15} />}
          title={t('settings.appearance.threadPreviewLine')}
          hint={t('settings.appearance.threadPreviewLineHint')}
          checked={threadPreviewLine}
          onChange={() => settings$.threadPreviewLine.set(!threadPreviewLine)}
        />
        <AppIconRows />
        <TrayIconRow />
        <TaskbarUnreadBadgeRow />
      </SettingsGroup>

      <SettingsGroup title={t('settings.sections.typography')}>
        <FontSettingsSection part="interface" />
      </SettingsGroup>

      <SettingsGroup title={t('settings.sections.sideNav')}>
        <ToggleRow
          icon={<Inbox size={15} />}
          title={t('settings.sideNav.showUnifiedInbox')}
          checked={showUnifiedInbox}
          onChange={() => setUnifiedInboxSideNavVisible(!showUnifiedInbox)}
        />
        {calendarEnabled && (
          <ToggleRow
            icon={<CalendarDays size={15} />}
            title={t('settings.sideNav.showCalendar')}
            checked={showCalendarInSideNav}
            onChange={() => setCalendarSideNavVisible(!showCalendarInSideNav)}
          />
        )}
        <ToggleRow
          icon={<Inbox size={15} />}
          title={t('settings.appearance.showUnreadAccountBadge')}
          hint={t('settings.appearance.showUnreadAccountBadgeHint')}
          checked={showUnreadAccountBadge}
          onChange={() => settings$.showUnreadAccountBadge.set(!showUnreadAccountBadge)}
        />
      </SettingsGroup>

      <SettingsGroup title={t('settings.sections.kanban')}>
        <NumberRow
          icon={<Columns3 size={15} />}
          title={t('settings.kanban.columnWidth')}
          value={String(kanbanColumnWidth)}
          min={KANBAN_COLUMN_MIN_WIDTH}
          max={KANBAN_COLUMN_MAX_WIDTH}
          step={10}
          suffix="px"
          onChange={(value) => {
            const width = Number(value)
            if (Number.isFinite(width)) settings$.kanbanColumnWidth.set(clampKanbanColumnWidth(width))
          }}
        />
        <ToggleRow
          icon={<Lock size={15} />}
          title={t('settings.kanban.lockScroll')}
          hint={t('settings.kanban.lockScrollHint')}
          checked={kanbanLockScroll}
          onChange={() => settings$.kanbanLockScroll.set(!kanbanLockScroll)}
        />
      </SettingsGroup>

      <ProxySettingsSection />

      <SettingsGroup title={t('settings.sections.app')}>
        <SettingRow
          title={t('shortcuts.title')}
          hint={t('shortcuts.customizeHint')}
          control={
            <button
              onClick={() => ui$.shortcutsOpen.set(true)}
              className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl bg-hover hover:bg-active text-primary font-bold text-[0.625rem] cursor-pointer transition-colors"
            >
              <Keyboard size={12} />
              {t('shortcuts.customize')}
            </button>
          }
        />
        <CloseToTrayRow />
        <IntegratedTitlebarRow />
        <AutoUpdateRow />
      </SettingsGroup>

      <SettingsGroup title={t('settings.sections.data')}>
        <SettingsBackupRow />
        <SettingsStorageRows />
        <SettingsLogsRow />
      </SettingsGroup>
    </div>
  )
}

// Offered where the backend can replace the app's icon: the running app's on
// macOS and Windows, the desktop entry's on Linux. The icon keeps its drawing
// and takes the picked colour.
function AppIconRows() {
  const { t } = useTranslation()
  const supported = useValue(appIconSupported$)
  const platform = useValue(appIconPlatform$)
  const custom = useValue(settings$.appIconCustom)
  const colour = useValue(settings$.appIconColour)

  if (!supported) return null

  return (
    <>
      <ToggleRow
        icon={<AppWindow size={15} />}
        title={t('settings.appearance.appIconCustom')}
        hint={t(appIconHintKey(platform))}
        checked={custom}
        onChange={() => settings$.appIconCustom.set(!custom)}
      />
      {custom && (
        <SettingRow
          title={t('settings.appearance.appIconColour')}
          control={
            // Applied live while the picker is dragged: setAppIcon keeps only
            // the latest colour in flight.
            <ColorSwatchPicker
              value={colour}
              onChange={(color) => settings$.appIconColour.set(parseAppIconColour(color))}
              onPreview={(color) => settings$.appIconColour.set(parseAppIconColour(color))}
              label={t('settings.appearance.appIconColour')}
            />
          }
        />
      )}
    </>
  )
}

// macOS and Linux only: Windows keeps the colour icon.
function TrayIconRow() {
  const { t } = useTranslation()
  const monochromeTrayIcon = useValue(settings$.monochromeTrayIcon)

  if (!offersMonochromeTrayIcon(navigator.userAgent, navigator.platform ?? '')) return null

  return (
    <ToggleRow
      icon={<Contrast size={15} />}
      title={t('settings.appearance.monochromeTrayIcon')}
      hint={t('settings.appearance.monochromeTrayIconHint')}
      checked={monochromeTrayIcon}
      onChange={() => settings$.monochromeTrayIcon.set(!monochromeTrayIcon)}
    />
  )
}

// Windows only: overlays the inbox unread count on the taskbar icon.
function TaskbarUnreadBadgeRow() {
  const { t } = useTranslation()
  const showTaskbarUnreadBadge = useValue(settings$.showTaskbarUnreadBadge)

  if (!isWindowsPlatform(navigator.userAgent, navigator.platform ?? '')) return null

  return (
    <ToggleRow
      icon={<Bell size={15} />}
      title={t('settings.appearance.showTaskbarUnreadBadge')}
      hint={t('settings.appearance.showTaskbarUnreadBadgeHint')}
      checked={showTaskbarUnreadBadge}
      onChange={() => settings$.showTaskbarUnreadBadge.set(!showTaskbarUnreadBadge)}
    />
  )
}

// macOS has its own split between closing a window and quitting (⌘Q), so the
// close button's behaviour is only a choice elsewhere.
function CloseToTrayRow() {
  const { t } = useTranslation()
  const closeToTray = useValue(settings$.closeToTray)

  if (isMac) return null

  return (
    <ToggleRow
      icon={<PanelTopClose size={15} />}
      title={t('settings.window.closeToTray')}
      hint={t('settings.window.closeToTrayHint', { quit: t('tray.quitMeron') })}
      checked={closeToTray}
      onChange={() => settings$.closeToTray.set(!closeToTray)}
    />
  )
}

// Linux and Windows. The choice lives in Go (window.json), since it is
// needed before the window exists. Windows and Linux desktops with native
// server-side decorations apply it on restart, so the switch shows the saved choice.
function IntegratedTitlebarRow() {
  const { t } = useTranslation()
  const supported = useValue(windowChrome$.supported)
  const wanted = useValue(windowChrome$.wanted)

  if (!supported) return null

  return (
    <ToggleRow
      icon={<PanelTop size={15} />}
      title={t('settings.window.integratedTitlebar')}
      hint={t('settings.window.integratedTitlebarHint')}
      checked={wanted}
      onChange={() =>
        void setIntegratedTitlebar(!wanted)
          .then(() => {
            if (windowChrome$.wanted.peek() !== windowChrome$.integrated.peek()) {
              showToast(t('settings.window.restartToApply'), 'success', 4000)
            }
          })
          .catch(() => {})
      }
    />
  )
}

// Only meaningful where the app can actually replace itself; store-managed and
// dev builds get no toggle at all rather than one that does nothing.
function AutoUpdateRow() {
  const { t } = useTranslation()
  const status = useValue(update$.status)
  const autoUpdateCheck = useValue(settings$.autoUpdateCheck)

  if (!status.supported) return null

  return (
    <ToggleRow
      icon={<RefreshCw size={15} />}
      title={t('settings.updates.autoCheck')}
      hint={t('settings.updates.autoCheckHint')}
      checked={autoUpdateCheck}
      onChange={() => settings$.autoUpdateCheck.set(!autoUpdateCheck)}
    />
  )
}
