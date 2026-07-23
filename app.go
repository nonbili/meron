package main

import (
	"context"
	"fmt"
	"log"
	"os"
	"path/filepath"
	"runtime"
	"sync"
	"time"

	wailsRuntime "github.com/wailsapp/wails/v2/pkg/runtime"
)

type App struct {
	ctx           context.Context
	sidecar       *Sidecar
	logger        *log.Logger
	logFile       *os.File
	trayMu        sync.Mutex
	trayHasUnread bool
	trayStop      func()
	trayStopOnce  sync.Once
	filePickerMu  sync.Mutex
	lastFileDir   string
	lastImageDir  string

	oauthMu          sync.Mutex
	oauthState       string
	oauthVerifier    string
	oauthProvider    string
	oauthRedirectURI string
	oauthProfile     *ExchangedProfile

	mailtoMu      sync.Mutex
	pendingMailto []string
}

func NewApp() *App {
	loadDotEnvLocal()
	logger, logFile := openAppLog()
	return &App{logger: logger, logFile: logFile}
}

func (a *App) Startup(ctx context.Context) {
	a.ctx = ctx
	a.queueStartupMailtoURLs(os.Args[1:])
	if wailsRuntime.Environment(ctx).BuildType != "dev" {
		installDesktopEntry()
	}
	a.startTray()
	a.logf("startup: core path=%s", coreBinaryPath())
	a.sidecar = NewSidecar(coreBinaryPath(), a.logWriter())
	a.sidecar.onEvent = a.handleSidecarEvent
	if err := a.sidecar.Start(ctx); err != nil {
		a.logf("core failed to start: %v", err)
		fmt.Fprintf(os.Stderr, "meron: core failed to start: %v (path: %s)\n", err, coreBinaryPath())
	} else {
		a.logf("core started")
	}
	a.setupNotificationListener()
	a.setupResumeListener()
}

// onSystemResumed tells the sidecar the host woke from suspend so its IDLE
// watchers drop sockets that died during sleep and reconnect immediately,
// instead of waiting out TCP keepalive / the IDLE timeout. Called from the
// per-platform resume listeners.
func (a *App) onSystemResumed() {
	a.logf("resume: system woke, kicking IDLE watchers")
	if a.sidecar != nil && a.sidecar.Started() {
		if _, err := a.sidecar.Call("system.resumed", nil); err != nil {
			a.logf("resume: system.resumed failed: %v", err)
		}
	}
}

func (a *App) HandleSecondInstanceLaunch(args []string) {
	for _, raw := range mailtoURLs(args) {
		a.openMailtoURL(raw)
	}
}

func (a *App) Shutdown(ctx context.Context) {
	a.logf("shutdown")
	a.stopTray()
	if a.sidecar != nil {
		a.sidecar.Close()
	}
	a.closeNotificationListener()
	a.closeResumeListener()
	if a.logFile != nil {
		_ = a.logFile.Close()
	}
}

func (a *App) Invoke(command string, payload map[string]any) (any, error) {
	start := time.Now()
	result, err := a.invoke(command, payload)
	if err != nil {
		a.logf("invoke %s failed after %s: %v", command, time.Since(start).Round(time.Millisecond), err)
	} else {
		a.logf("invoke %s ok after %s", command, time.Since(start).Round(time.Millisecond))
	}
	return result, err
}

func (a *App) invoke(command string, payload map[string]any) (any, error) {
	if payload == nil {
		payload = map[string]any{}
	}
	switch command {
	case "system.check":
		return a.systemCheck()
	case "system.pickImageFile":
		return a.pickImageFile(payload)
	case "system.pickFiles":
		return a.pickFiles(payload, false, true)
	case "system.pickImageFiles":
		return a.pickFiles(payload, true, true)
	case "mailto.consumePending":
		return a.consumePendingMailto(), nil
	case "app.prefsGet":
		return a.appPrefsGet(payload)
	case "app.prefsSet":
		return a.appPrefsSet(payload)
	case "tray.setUnread":
		return a.traySetUnread(payload)
	case "changelog.fetch":
		return a.changelogFetch()
	case "log.read":
		return a.logRead()
	case "log.export":
		return a.logExport()
	case "storage.usage":
		return a.storageUsage()
	case "storage.clearCache":
		return a.storageClearCache()
	case "account.list":
		return a.accountList()
	case "account.addPassword":
		return a.accountAddPassword(payload)
	case "account.autodiscover":
		return a.autodiscover(payload)
	case "account.addGmailOAuth", "oauth.gmailComplete":
		return a.accountAddGmailOAuth(payload)
	case "account.addOutlookOAuth", "oauth.outlookComplete":
		return a.accountAddOutlookOAuth(payload)
	case "account.addRSS":
		return a.accountAddRSS(payload)
	case "rss.addFeed":
		return a.feedAdd(payload)
	case "rss.removeFeed":
		return a.feedRemove(payload)
	case "rss.moveFeed":
		return a.feedMove(payload)
	case "rss.exportOpml":
		return a.exportOpml(payload)
	case "rss.importOpml":
		return a.importOpml(payload)
	case "account.remove":
		return a.accountRemove(payload)
	case "account.setImages":
		return a.accountSetImages(payload)
	case "account.setConversationHtml":
		return a.accountSetPref(payload, "account.setConversationHtml")
	case "account.setChatWallpaper":
		return a.accountSetChatWallpaper(payload)
	case "account.setName":
		return a.accountSetName(payload)
	case "account.setSenderName":
		return a.accountSetSenderName(payload)
	case "account.setAvatar":
		return a.accountSetAvatar(payload)
	case "account.setAliases":
		return a.accountSetAliases(payload)
	case "account.setUnified":
		return a.accountSetPref(payload, "account.setUnified")
	case "account.setMuted":
		return a.accountSetPref(payload, "account.setMuted")
	case "account.setPaused":
		return a.accountSetPref(payload, "account.setPaused")
	case "account.setSaveSentCopy":
		return a.accountSetSaveSentCopy(payload)
	case "account.setRSSSyncInterval":
		return a.accountSetRSSSyncInterval(payload)
	case "account.reorder":
		return a.accountReorder(payload)
	case "oauth.gmailBegin":
		return a.gmailBegin()
	case "oauth.outlookBegin":
		return a.outlookBegin()
	case "oauth.gmailPollProfile", "oauth.outlookPollProfile":
		return a.oauthPollProfile(), nil
	case "mail.sync":
		return a.mailSync(payload)
	case "watch.start", "watch.stop":
		return a.watchFolder(command, payload)
	case "mail.folderList":
		return a.folderList(payload)
	case "mail.folderCreate":
		return a.folderCreate(payload)
	case "mail.threadList", "mail.search":
		return a.threadList(payload)
	case "mail.starredItems":
		return a.starredItems(payload)
	case "mail.allocateIdentity":
		if a.sidecar == nil || !a.sidecar.Started() {
			return nil, fmt.Errorf("mail engine unavailable")
		}
		return a.sidecar.Call("identity.allocate", payload)
	case "mail.suggestContacts":
		return a.suggestContacts(payload)
	case "mail.threadRead":
		return a.threadRead(payload)
	case "mail.send":
		return a.mailSend(payload)
	case "mail.saveDraft":
		return a.mailSaveDraft(payload)
	case "mail.discardDraft":
		return a.mailDiscardDraft(payload)
	case "mail.markRead":
		return a.markRead(payload)
	case "mail.markStarred":
		return a.markStarred(payload)
	case "mail.markAllRead":
		return a.markAllRead(payload)
	case "mail.move":
		return a.mailMove(payload)
	case "mail.copy":
		return a.mailCopy(payload)
	case "mail.saveAttachment":
		return a.saveAttachment(payload)
	case "mail.copyImage":
		return a.copyImage(payload)
	case "mail.readAttachment":
		return a.readAttachment(payload)
	case "composer.readClipboardImage":
		return a.readClipboardImageAttachment(payload)
	case "composer.writeMediaFile":
		return a.writeMediaFile(payload)
	case "composer.openAttachment":
		return a.openComposerAttachment(payload)
	case "account.writeAvatarFile":
		return a.writeAvatarFile(payload)
	case "account.writeChatWallpaperFile":
		return a.writeChatWallpaperFile(payload)
	case "composer.pruneMedia":
		return a.pruneComposerMedia(payload)
	case "mail.archive":
		return a.mailArchive(payload)
	case "mail.delete":
		return a.mailDelete(payload)
	default:
		return nil, fmt.Errorf("unknown command: %s", command)
	}
}

func (a *App) appPrefsGet(payload map[string]any) (any, error) {
	if a.sidecar == nil || !a.sidecar.Started() {
		return map[string]any{"prefs": map[string]any{}}, nil
	}
	return a.sidecar.Call("app.prefsGet", payload)
}

func (a *App) appPrefsSet(payload map[string]any) (any, error) {
	if a.sidecar == nil || !a.sidecar.Started() {
		return map[string]any{"ok": true}, nil
	}
	return a.sidecar.Call("app.prefsSet", payload)
}

func (a *App) systemCheck() (any, error) {
	dbPath := filepath.Join(appConfigDir(), "meron.db")
	return map[string]any{
		"platform":    runtime.GOOS,
		"mail_engine": "meron_mail",
		"meron_mail": map[string]any{
			"configured":  coreBinaryPath() != "",
			"available":   fileExists(coreBinaryPath()),
			"server_path": coreBinaryPath(),
		},
		"gmail_oauth_configured":   gmailOAuthConfigured(),
		"outlook_oauth_configured": outlookOAuthConfigured(),
		"database_path":            dbPath,
		"log_path":                 filepath.Join(appConfigDir(), "meron.log"),
	}, nil
}
