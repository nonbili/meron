package main

import (
	"bufio"
	"bytes"
	"context"
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"log"
	"net"
	"net/http"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"runtime"
	"strconv"
	"strings"
	"sync"
	"time"

	wailsRuntime "github.com/wailsapp/wails/v2/pkg/runtime"
)

// installDesktopEntry writes a .desktop file and icon into the user's
// XDG data dir so GNOME/KDE/etc. show the app's logo in the dock and
// switcher, and registers Meron as the user's mailto: handler. Safe to
// call on every startup: it only rewrites files when their content has changed.
func installDesktopEntry() {
	if runtime.GOOS != "linux" {
		return
	}

	exe, err := os.Executable()
	if err != nil {
		return
	}
	exe, err = filepath.EvalSymlinks(exe)
	if err != nil {
		return
	}

	dataHome := os.Getenv("XDG_DATA_HOME")
	if dataHome == "" {
		home, err := os.UserHomeDir()
		if err != nil {
			return
		}
		dataHome = filepath.Join(home, ".local", "share")
	}

	iconPath := filepath.Join(dataHome, "icons", "meron.png")
	desktopPath := filepath.Join(dataHome, "applications", "meron.desktop")

	writeIfChanged(iconPath, appIconPNG)

	desktop := fmt.Sprintf(`[Desktop Entry]
Type=Application
Name=Meron
Comment=Messages that spark joy
Exec=%s %%u
Icon=meron
Terminal=false
Categories=Office;Network;Email;
StartupWMClass=meron
MimeType=x-scheme-handler/mailto;
`, desktopExecArg(exe))
	writeIfChanged(desktopPath, []byte(desktop))

	// Refresh the MIME associations database so the mailto: handler takes
	// effect without a re-login. No-op on hosts without the utility.
	if bin, err := exec.LookPath("update-desktop-database"); err == nil {
		appsDir := filepath.Join(dataHome, "applications")
		go func() { _ = exec.Command(bin, appsDir).Run() }()
	}
}

func writeIfChanged(path string, data []byte) {
	if existing, err := os.ReadFile(path); err == nil {
		if sha256.Sum256(existing) == sha256.Sum256(data) {
			return
		}
	}
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return
	}
	_ = os.WriteFile(path, data, 0o644)
}

func desktopExecArg(path string) string {
	if strings.ContainsAny(path, " \t\n\"'\\") {
		return strconv.Quote(path)
	}
	return path
}

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
		return a.pickFiles(payload, false)
	case "system.pickImageFiles":
		return a.pickFiles(payload, true)
	case "mailto.consumePending":
		return a.consumePendingMailto(), nil
	case "app.prefsGet":
		return a.appPrefsGet(payload)
	case "app.prefsSet":
		return a.appPrefsSet(payload)
	case "tray.setUnread":
		return a.traySetUnread(payload)
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

func (a *App) queueStartupMailtoURLs(args []string) {
	urls := mailtoURLs(args)
	if len(urls) == 0 {
		return
	}
	a.mailtoMu.Lock()
	a.pendingMailto = append(a.pendingMailto, urls...)
	a.mailtoMu.Unlock()
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

func (a *App) pickImageFile(payload map[string]any) (any, error) {
	res, err := a.pickFiles(payload, true)
	if err != nil {
		return nil, err
	}
	resMap, _ := res.(map[string]any)
	if cancelled, _ := resMap["cancelled"].(bool); cancelled {
		return resMap, nil
	}
	files, _ := resMap["files"].([]any)
	if len(files) == 0 {
		return map[string]any{"cancelled": true}, nil
	}
	return files[0], nil
}

func (a *App) pickFiles(payload map[string]any, imagesOnly bool) (any, error) {
	title, _ := payload["title"].(string)
	if title == "" {
		if imagesOnly {
			title = "Choose image"
		} else {
			title = "Choose files"
		}
	}

	a.filePickerMu.Lock()
	defaultDir := a.lastFileDir
	lastDirPath := lastFileDirPath()
	if imagesOnly {
		defaultDir = a.lastImageDir
		lastDirPath = lastImageDirPath()
	}
	if defaultDir == "" {
		if data, err := os.ReadFile(lastDirPath); err == nil {
			defaultDir = strings.TrimSpace(string(data))
			if imagesOnly {
				a.lastImageDir = defaultDir
			} else {
				a.lastFileDir = defaultDir
			}
		}
	}
	a.filePickerMu.Unlock()

	defaultDir = filePickerDefaultDir(defaultDir)

	options := wailsRuntime.OpenDialogOptions{
		Title:            title,
		DefaultDirectory: defaultDir,
	}
	if imagesOnly {
		options.Filters = []wailsRuntime.FileFilter{
			{DisplayName: "Image files (*.png, *.jpg, *.jpeg, *.webp, *.gif)", Pattern: "*.png;*.jpg;*.jpeg;*.webp;*.gif"},
		}
	}

	paths, err := wailsRuntime.OpenMultipleFilesDialog(a.ctx, options)
	if err != nil {
		return nil, err
	}
	if len(paths) == 0 {
		return map[string]any{"cancelled": true}, nil
	}

	files := make([]any, 0, len(paths))
	for _, path := range paths {
		data, err := os.ReadFile(path)
		if err != nil {
			return nil, fmt.Errorf("read file: %w", err)
		}
		mime := http.DetectContentType(data)
		if imagesOnly && !strings.HasPrefix(mime, "image/") {
			return nil, errors.New("selected file is not an image")
		}
		files = append(files, map[string]any{
			"name": filepath.Base(path),
			"mime": mime,
			"data": "data:" + mime + ";base64," + base64.StdEncoding.EncodeToString(data),
		})
	}

	a.filePickerMu.Lock()
	nextDir := filepath.Dir(paths[0])
	if imagesOnly {
		a.lastImageDir = nextDir
	} else {
		a.lastFileDir = nextDir
	}
	if err := os.MkdirAll(filepath.Dir(lastDirPath), 0o755); err == nil {
		_ = os.WriteFile(lastDirPath, []byte(nextDir), 0o600)
	}
	a.filePickerMu.Unlock()

	return map[string]any{"files": files}, nil
}

func lastImageDirPath() string {
	return filepath.Join(appConfigDir(), "last-image-dir")
}

func lastFileDirPath() string {
	return filepath.Join(appConfigDir(), "last-file-dir")
}

func filePickerDefaultDir(candidate string) string {
	if dir, ok := usablePickerDir(candidate); ok {
		return dir
	}
	home, err := os.UserHomeDir()
	if err == nil {
		if dir, ok := usablePickerDir(home); ok {
			return dir
		}
	}
	return ""
}

func usablePickerDir(path string) (string, bool) {
	if path == "" {
		return "", false
	}
	info, err := os.Stat(path)
	if err != nil || !info.IsDir() {
		return "", false
	}
	realPath, err := filepath.EvalSymlinks(path)
	if err == nil && realPath != "" {
		return realPath, true
	}
	return path, true
}

func (a *App) consumePendingMailto() []string {
	a.mailtoMu.Lock()
	defer a.mailtoMu.Unlock()
	urls := append([]string(nil), a.pendingMailto...)
	a.pendingMailto = nil
	if urls == nil {
		return []string{}
	}
	return urls
}

func (a *App) openMailtoURL(raw string) {
	if !isMailtoURL(raw) {
		return
	}
	if a.ctx == nil {
		a.queueStartupMailtoURLs([]string{raw})
		return
	}
	wailsRuntime.WindowShow(a.ctx)
	wailsRuntime.WindowUnminimise(a.ctx)
	wailsRuntime.EventsEmit(a.ctx, "mailto.open", raw)
}

func mailtoURLs(args []string) []string {
	urls := make([]string, 0, len(args))
	for _, arg := range args {
		if isMailtoURL(arg) {
			urls = append(urls, arg)
		}
	}
	return urls
}

func isMailtoURL(raw string) bool {
	u, err := url.Parse(strings.TrimSpace(raw))
	return err == nil && strings.EqualFold(u.Scheme, "mailto")
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

func (a *App) accountAddPassword(payload map[string]any) (any, error) {
	var req AddPasswordAccountRequest
	if err := decode(payload, &req); err != nil {
		return nil, err
	}
	if !strings.Contains(req.Email, "@") {
		return nil, errors.New("invalid email")
	}
	if req.IMAPHost == "" || req.SMTPHost == "" {
		return nil, errors.New("server required")
	}
	if req.Username == "" || req.Password == "" {
		return nil, errors.New("credentials required")
	}
	if req.IMAPPort == 0 {
		req.IMAPPort = 993
	}
	if req.SMTPPort == 0 {
		req.SMTPPort = 465
	}
	id := accountID(req.Email)
	a.logf("account.addPassword: connecting account=%s imap=%s:%d smtp=%s:%d", id, req.IMAPHost, req.IMAPPort, req.SMTPHost, req.SMTPPort)
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	imapTLS, imapStartTLS := tlsMode(req.TLS, req.IMAPPort)
	smtpTLS, smtpStartTLS := tlsMode(req.TLS, req.SMTPPort)
	if _, err := a.sidecar.Call("account.connect", map[string]any{
		"id":            id,
		"host":          req.IMAPHost,
		"port":          req.IMAPPort,
		"user":          req.Username,
		"password":      req.Password,
		"tls":           imapTLS,
		"starttls":      imapStartTLS,
		"smtp_host":     req.SMTPHost,
		"smtp_port":     req.SMTPPort,
		"smtp_tls":      smtpTLS,
		"smtp_starttls": smtpStartTLS,
		"email":         req.Email,
		"display_name":  req.DisplayName,
		"sender_name":   req.SenderName,
		"provider":      "custom",
	}); err != nil {
		return nil, err
	}
	_, _ = a.sidecar.Call("watch.start", map[string]any{"account": id})

	account := Account{
		ID:                id,
		Email:             req.Email,
		DisplayName:       req.DisplayName,
		SenderName:        req.SenderName,
		Provider:          "custom",
		AuthType:          "password",
		IncludedInUnified: true,
		IMAPHost:          req.IMAPHost,
		IMAPPort:          req.IMAPPort,
		SMTPHost:          req.SMTPHost,
		SMTPPort:          req.SMTPPort,
		TLS:               req.TLS,
	}
	return map[string]any{"account": account}, nil
}

func (a *App) accountAddGmailOAuth(payload map[string]any) (any, error) {
	var req AddGmailOAuthRequest
	if err := decode(payload, &req); err != nil {
		return nil, err
	}
	if googleClientID() == "" {
		return nil, errors.New("Google client ID missing")
	}
	if googleClientSecret() == "" {
		return nil, errors.New("Google client secret missing")
	}
	if req.AccessToken == "" || req.RefreshToken == "" {
		a.logf("account.addGmailOAuth: exchanging auth code")
		tokens, err := a.exchangeGmailOAuthCode(req.AuthCode)
		if err != nil {
			return nil, err
		}
		req.AccessToken = tokens.AccessToken
		req.RefreshToken = tokens.RefreshToken
		req.ExpiresIn = tokens.ExpiresIn
	}
	if req.Email == "" || req.AvatarURL == "" {
		profile, err := fetchGoogleUserInfo(req.AccessToken)
		if err != nil {
			return nil, err
		}
		if req.Email == "" {
			req.Email = profile.Email
		}
		if req.DisplayName == "" {
			req.DisplayName = profile.Name
		}
		if req.AvatarURL == "" {
			req.AvatarURL = profile.Picture
		}
	}
	if !strings.Contains(req.Email, "@") {
		return nil, errors.New("invalid email")
	}

	id := accountID(req.Email)
	if req.AvatarURL != "" {
		req.AvatarURL = a.downloadAndSaveAvatar(id, req.AvatarURL)
	}
	expiresAt := time.Now().Unix() + req.ExpiresIn
	a.logf("account.addGmailOAuth: connecting account=%s email=%s token_present=%t refresh_present=%t", id, req.Email, req.AccessToken != "", req.RefreshToken != "")
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	if _, err := a.sidecar.Call("account.connect", map[string]any{
		"id":               id,
		"host":             "imap.gmail.com",
		"port":             993,
		"user":             req.Email,
		"password":         "",
		"tls":              true,
		"smtp_host":        "smtp.gmail.com",
		"smtp_port":        465,
		"smtp_tls":         true,
		"auth_type":        "gmail_oauth",
		"access_token":     req.AccessToken,
		"refresh_token":    req.RefreshToken,
		"token_expires_at": expiresAt,
		"validate":         false,
		"email":            req.Email,
		"display_name":     req.DisplayName,
		"sender_name":      req.SenderName,
		"avatar_url":       req.AvatarURL,
		"provider":         "gmail",
	}); err != nil {
		return nil, err
	}
	_, _ = a.sidecar.Call("watch.start", map[string]any{"account": id})

	account := Account{
		ID:                id,
		Email:             req.Email,
		DisplayName:       req.DisplayName,
		SenderName:        req.SenderName,
		AvatarURL:         req.AvatarURL,
		Provider:          "gmail",
		AuthType:          "gmail_oauth",
		IncludedInUnified: true,
		IMAPHost:          "imap.gmail.com",
		IMAPPort:          993,
		SMTPHost:          "smtp.gmail.com",
		SMTPPort:          465,
		TLS:               true,
		AccessToken:       req.AccessToken,
		RefreshToken:      req.RefreshToken,
		TokenExpiresAt:    expiresAt,
	}
	a.logf("account.addGmailOAuth: saved account=%s", id)
	return map[string]any{"account": account}, nil
}

func (a *App) accountAddOutlookOAuth(payload map[string]any) (any, error) {
	var req AddOutlookOAuthRequest
	if err := decode(payload, &req); err != nil {
		return nil, err
	}
	if outlookClientID() == "" {
		return nil, errors.New("Outlook client ID missing")
	}
	if req.AccessToken == "" || req.RefreshToken == "" {
		a.logf("account.addOutlookOAuth: exchanging auth code")
		a.oauthMu.Lock()
		verifier := a.oauthVerifier
		redirectURI := a.oauthRedirectURI
		a.oauthVerifier = ""
		a.oauthMu.Unlock()
		tokens, err := exchangeOutlookOAuthCodeWithVerifier(req.AuthCode, verifier, redirectURI)
		if err != nil {
			return nil, err
		}
		req.AccessToken = tokens.AccessToken
		req.RefreshToken = tokens.RefreshToken
		req.ExpiresIn = tokens.ExpiresIn
		if req.Email == "" {
			email, name := parseIDTokenClaims(tokens.IDToken)
			req.Email = email
			if req.DisplayName == "" {
				req.DisplayName = name
			}
		}
	}
	if !strings.Contains(req.Email, "@") {
		return nil, errors.New("invalid email")
	}

	id := accountID(req.Email)
	expiresAt := time.Now().Unix() + req.ExpiresIn
	a.logf("account.addOutlookOAuth: connecting account=%s email=%s token_present=%t refresh_present=%t", id, req.Email, req.AccessToken != "", req.RefreshToken != "")
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	if _, err := a.sidecar.Call("account.connect", map[string]any{
		"id":               id,
		"host":             "outlook.office365.com",
		"port":             993,
		"user":             req.Email,
		"password":         "",
		"tls":              true,
		"smtp_host":        "smtp-mail.outlook.com",
		"smtp_port":        587,
		"smtp_tls":         true,
		"smtp_starttls":    true,
		"auth_type":        "outlook_oauth",
		"access_token":     req.AccessToken,
		"refresh_token":    req.RefreshToken,
		"token_expires_at": expiresAt,
		"validate":         false,
		"email":            req.Email,
		"display_name":     req.DisplayName,
		"sender_name":      req.SenderName,
		"provider":         "outlook",
	}); err != nil {
		return nil, err
	}
	_, _ = a.sidecar.Call("watch.start", map[string]any{"account": id})

	account := Account{
		ID:                id,
		Email:             req.Email,
		DisplayName:       req.DisplayName,
		SenderName:        req.SenderName,
		Provider:          "outlook",
		AuthType:          "outlook_oauth",
		IncludedInUnified: true,
		IMAPHost:          "outlook.office365.com",
		IMAPPort:          993,
		SMTPHost:          "smtp-mail.outlook.com",
		SMTPPort:          587,
		TLS:               true,
		AccessToken:       req.AccessToken,
		RefreshToken:      req.RefreshToken,
		TokenExpiresAt:    expiresAt,
	}
	a.logf("account.addOutlookOAuth: saved account=%s", id)
	return map[string]any{"account": account}, nil
}

func (a *App) accountAddRSS(payload map[string]any) (any, error) {
	feedURL, _ := payload["feed_url"].(string)
	displayName, _ := payload["display_name"].(string)
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	res, err := a.sidecar.Call("account.addRss", map[string]any{
		"feed_url":     feedURL,
		"display_name": displayName,
	})
	if err != nil {
		return nil, err
	}
	account := decodeAccount(res)
	a.logf("account.addRSS: saved feed account=%s", account.ID)
	return map[string]any{"account": account}, nil
}

func (a *App) feedAdd(payload map[string]any) (any, error) {
	accountID, _ := payload["account_id"].(string)
	feedURL, _ := payload["feed_url"].(string)
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	res, err := a.sidecar.Call("feed.add", map[string]any{"account": accountID, "feed_url": feedURL})
	if err != nil {
		return nil, err
	}
	a.logf("rss.addFeed: added feed to account=%s", accountID)
	return res, nil
}

func (a *App) feedRemove(payload map[string]any) (any, error) {
	threadID, _ := payload["thread_id"].(string)
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	res, err := a.sidecar.Call("feed.remove", map[string]any{"thread_id": threadID})
	if err != nil {
		return nil, err
	}
	a.logf("rss.removeFeed: removed feed thread=%s", threadID)
	return res, nil
}

func (a *App) feedMove(payload map[string]any) (any, error) {
	threadID, _ := payload["thread_id"].(string)
	targetAccount, _ := payload["target_account_id"].(string)
	if targetAccount == "" {
		targetAccount, _ = payload["target_account"].(string)
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	res, err := a.sidecar.Call("feed.move", map[string]any{"thread_id": threadID, "target_account": targetAccount})
	if err != nil {
		return nil, err
	}
	a.logf("rss.moveFeed: moved feed thread=%s target=%s", threadID, targetAccount)
	return res, nil
}

// exportOpml asks the sidecar for an OPML document of one RSS account's feeds,
// then writes it to a user-chosen path via a native save dialog.
func (a *App) exportOpml(payload map[string]any) (any, error) {
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	account, _ := payload["account"].(string)
	if account == "" {
		return nil, errors.New("account id required")
	}
	res, err := a.sidecar.Call("rss.exportOpml", map[string]any{"account": account})
	if err != nil {
		return nil, err
	}
	resMap, _ := res.(map[string]any)
	opml, _ := resMap["opml"].(string)
	if opml == "" {
		return nil, errors.New("no feeds to export")
	}

	dest, err := wailsRuntime.SaveFileDialog(a.ctx, wailsRuntime.SaveDialogOptions{
		Title:                "Export feeds",
		DefaultFilename:      "meron-feeds.opml",
		CanCreateDirectories: true,
		Filters: []wailsRuntime.FileFilter{
			{DisplayName: "OPML files (*.opml)", Pattern: "*.opml"},
		},
	})
	if err != nil {
		return nil, err
	}
	if dest == "" {
		return map[string]any{"saved": false}, nil // user cancelled
	}
	if err := os.WriteFile(dest, []byte(opml), 0o644); err != nil {
		return nil, fmt.Errorf("write OPML: %w", err)
	}
	a.logf("rss.exportOpml: wrote %s", dest)
	return map[string]any{"saved": true, "path": dest}, nil
}

// importOpml reads a user-chosen OPML file via a native open dialog and hands its
// contents to the sidecar, which adds the feeds to the given RSS account. Returns
// the number of feeds added.
func (a *App) importOpml(payload map[string]any) (any, error) {
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	account, _ := payload["account"].(string)
	if account == "" {
		return nil, errors.New("account id required")
	}
	src, err := wailsRuntime.OpenFileDialog(a.ctx, wailsRuntime.OpenDialogOptions{
		Title: "Import feeds",
		Filters: []wailsRuntime.FileFilter{
			{DisplayName: "OPML files (*.opml, *.xml)", Pattern: "*.opml;*.xml"},
		},
	})
	if err != nil {
		return nil, err
	}
	if src == "" {
		return map[string]any{"imported": 0, "cancelled": true}, nil // user cancelled
	}
	data, err := os.ReadFile(src)
	if err != nil {
		return nil, fmt.Errorf("read OPML: %w", err)
	}

	res, err := a.sidecar.Call("rss.importOpml", map[string]any{"opml": string(data), "account": account})
	if err != nil {
		return nil, err
	}
	resMap, _ := res.(map[string]any)
	imported := 0
	if n, ok := resMap["imported"].(float64); ok {
		imported = int(n)
	}
	a.logf("rss.importOpml: imported %d feeds from %s", imported, src)
	return map[string]any{"imported": imported}, nil
}

func (a *App) accountRemove(payload map[string]any) (any, error) {
	id, _ := payload["id"].(string)
	// The unified DB is the single source of truth: the sidecar deletes the
	// account row and cascades its cached state (mail folders/messages or rss
	// subscriptions/items), plus the keychain secret.
	if a.sidecar != nil && a.sidecar.Started() {
		if _, err := a.sidecar.Call("account.remove", map[string]any{"id": id}); err != nil {
			a.logf("account.remove: sidecar cleanup failed for %s: %v", id, err)
		}
	}
	return map[string]any{"ok": true}, nil
}

func (a *App) accountSetImages(payload map[string]any) (any, error) {
	id, _ := payload["id"].(string)
	if id == "" {
		id, _ = payload["account_id"].(string)
	}
	if id == "" {
		return nil, errors.New("account id required")
	}
	enabled, _ := payload["enabled"].(bool)
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	if _, err := a.sidecar.Call("account.setImages", map[string]any{"account": id, "enabled": enabled}); err != nil {
		return nil, err
	}
	a.logf("account.setImages: account=%s enabled=%t", id, enabled)
	return map[string]any{"ok": true}, nil
}

func (a *App) accountReorder(payload map[string]any) (any, error) {
	accounts, _ := payload["accounts"].([]any)
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	if _, err := a.sidecar.Call("account.reorder", map[string]any{"accounts": accounts}); err != nil {
		return nil, err
	}
	return map[string]any{"ok": true}, nil
}

func (a *App) accountSetName(payload map[string]any) (any, error) {
	id, _ := payload["id"].(string)
	if id == "" {
		id, _ = payload["account_id"].(string)
	}
	if id == "" {
		return nil, errors.New("account id required")
	}
	name, _ := payload["name"].(string)
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	if _, err := a.sidecar.Call("account.setName", map[string]any{"account": id, "name": name}); err != nil {
		return nil, err
	}
	a.logf("account.setName: account=%s name=%s", id, name)
	return map[string]any{"ok": true}, nil
}

func (a *App) accountSetSenderName(payload map[string]any) (any, error) {
	id, _ := payload["id"].(string)
	if id == "" {
		id, _ = payload["account_id"].(string)
	}
	if id == "" {
		return nil, errors.New("account id required")
	}
	name, _ := payload["name"].(string)
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	if _, err := a.sidecar.Call("account.setSenderName", map[string]any{"account": id, "name": name}); err != nil {
		return nil, err
	}
	a.logf("account.setSenderName: account=%s name=%s", id, name)
	return map[string]any{"ok": true}, nil
}

func (a *App) accountSetAvatar(payload map[string]any) (any, error) {
	id, _ := payload["id"].(string)
	if id == "" {
		id, _ = payload["account_id"].(string)
	}
	if id == "" {
		return nil, errors.New("account id required")
	}
	avatarURL, _ := payload["avatar_url"].(string)
	avatarURL = a.downloadAndSaveAvatar(id, avatarURL)
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	if _, err := a.sidecar.Call("account.setAvatar", map[string]any{"account": id, "avatar_url": strings.TrimSpace(avatarURL)}); err != nil {
		return nil, err
	}
	a.logf("account.setAvatar: account=%s", id)
	return map[string]any{"ok": true}, nil
}

// accountSetAliases replaces the whole send-as alias list for an account. The
// sidecar normalizes entries (trim, drop blank emails, dedupe).
func (a *App) accountSetAliases(payload map[string]any) (any, error) {
	id, _ := payload["id"].(string)
	if id == "" {
		id, _ = payload["account_id"].(string)
	}
	if id == "" {
		return nil, errors.New("account id required")
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	aliases := payload["aliases"]
	if aliases == nil {
		aliases = []any{}
	}
	if _, err := a.sidecar.Call("account.setAliases", map[string]any{"account": id, "aliases": aliases}); err != nil {
		return nil, err
	}
	a.logf("account.setAliases: account=%s", id)
	return map[string]any{"ok": true}, nil
}

// accountSetPref forwards a boolean per-account toggle (unified-inbox inclusion,
// mute, pause) to the sidecar, which persists it and applies any side effects
// (e.g. pausing stops the IDLE watcher). `method` is the sidecar method name.
func (a *App) accountSetPref(payload map[string]any, method string) (any, error) {
	id, _ := payload["id"].(string)
	if id == "" {
		id, _ = payload["account_id"].(string)
	}
	if id == "" {
		return nil, errors.New("account id required")
	}
	enabled, _ := payload["enabled"].(bool)
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	if _, err := a.sidecar.Call(method, map[string]any{"account": id, "enabled": enabled}); err != nil {
		return nil, err
	}
	a.logf("%s: account=%s enabled=%t", method, id, enabled)
	return map[string]any{"ok": true}, nil
}

func (a *App) accountSetChatWallpaper(payload map[string]any) (any, error) {
	id, _ := payload["id"].(string)
	if id == "" {
		id, _ = payload["account_id"].(string)
	}
	if id == "" {
		return nil, errors.New("account id required")
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	wallpaper, ok := payload["wallpaper"]
	if !ok {
		wallpaper = nil
	}
	if _, err := a.sidecar.Call("account.setChatWallpaper", map[string]any{"account": id, "wallpaper": wallpaper}); err != nil {
		return nil, err
	}
	a.logf("account.setChatWallpaper: account=%s", id)
	return map[string]any{"ok": true}, nil
}

func (a *App) accountSetRSSSyncInterval(payload map[string]any) (any, error) {
	id, _ := payload["id"].(string)
	if id == "" {
		id, _ = payload["account_id"].(string)
	}
	if id == "" {
		return nil, errors.New("account id required")
	}
	minutes, ok := numberPayload(payload["minutes"])
	if !ok {
		return nil, errors.New("minutes required")
	}
	if minutes < 5 {
		minutes = 5
	} else if minutes > 1440 {
		minutes = 1440
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	if _, err := a.sidecar.Call("account.setRSSSyncInterval", map[string]any{"account": id, "minutes": minutes}); err != nil {
		return nil, err
	}
	a.logf("account.setRSSSyncInterval: account=%s minutes=%d", id, minutes)
	return map[string]any{"ok": true, "minutes": minutes}, nil
}

func numberPayload(v any) (int, bool) {
	switch n := v.(type) {
	case int:
		return n, true
	case int64:
		return int(n), true
	case float64:
		return int(n), true
	case float32:
		return int(n), true
	default:
		return 0, false
	}
}

func (a *App) gmailBegin() (any, error) {
	if googleClientID() == "" {
		return nil, errors.New("Google client ID missing")
	}
	if googleClientSecret() == "" {
		return nil, errors.New("Google client secret missing")
	}
	return a.beginOAuth("gmail")
}

func (a *App) outlookBegin() (any, error) {
	if outlookClientID() == "" {
		return nil, errors.New("Outlook client ID missing")
	}
	return a.beginOAuth("outlook")
}

// beginOAuth runs the shared PKCE authorization-code setup for any provider:
// it generates state + verifier, arms the loopback callback listener, builds
// the provider's authorization URL, and opens the system browser. The active
// provider is recorded so the (shared) callback handler knows which token
// endpoint and profile source to use.
func (a *App) beginOAuth(provider string) (any, error) {
	state := randomBase64URL(32)
	verifier := randomBase64URL(32)
	challengeBytes := sha256.Sum256([]byte(verifier))
	challenge := base64.RawURLEncoding.EncodeToString(challengeBytes[:])

	// Bind an ephemeral loopback port up front so the redirect URI carries the
	// real port (RFC 8252). The same URI must be used at the token exchange, so
	// it's stored on the App for the callback handler / fallback path to read.
	listener, err := net.Listen("tcp", oauthLoopbackHost+":0")
	if err != nil {
		return nil, fmt.Errorf("start OAuth callback listener: %w", err)
	}
	redirectURI := "http://" + listener.Addr().String()

	a.oauthMu.Lock()
	a.oauthState = state
	a.oauthVerifier = verifier
	a.oauthProvider = provider
	a.oauthRedirectURI = redirectURI
	a.oauthProfile = nil
	a.oauthMu.Unlock()

	a.logf("oauth.begin: provider=%s callback listening on %s", provider, redirectURI)
	go a.serveOAuthRedirect(listener)

	values := url.Values{}
	values.Set("response_type", "code")
	values.Set("redirect_uri", redirectURI)
	values.Set("state", state)
	values.Set("code_challenge", challenge)
	values.Set("code_challenge_method", "S256")

	var authBase string
	switch provider {
	case "outlook":
		values.Set("client_id", outlookClientID())
		values.Set("scope", outlookScopes)
		authBase = "https://login.microsoftonline.com/common/oauth2/v2.0/authorize"
	default: // gmail
		values.Set("client_id", googleClientID())
		values.Set("scope", "https://mail.google.com/ https://www.googleapis.com/auth/userinfo.email https://www.googleapis.com/auth/userinfo.profile")
		values.Set("access_type", "offline")
		values.Set("prompt", "consent")
		authBase = "https://accounts.google.com/o/oauth2/v2/auth"
	}
	authURL := authBase + "?" + values.Encode()
	if a.ctx != nil {
		wailsRuntime.BrowserOpenURL(a.ctx, authURL)
	}
	a.logf("oauth.begin: provider=%s opened external browser", provider)
	return map[string]any{"url": authURL, "needs_external_browser": true}, nil
}

func (a *App) oauthPollProfile() any {
	a.oauthMu.Lock()
	defer a.oauthMu.Unlock()
	if a.oauthProfile == nil {
		return map[string]any{"exchanged": false}
	}
	profile := a.oauthProfile
	a.oauthProfile = nil
	a.logf("oauth.pollProfile: returning exchanged profile for %s", profile.Email)
	return map[string]any{"exchanged": true, "profile": profile}
}

func (a *App) serveOAuthRedirect(listener net.Listener) {
	defer listener.Close()

	server := &http.Server{ReadHeaderTimeout: 10 * time.Second}
	server.Handler = http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		a.oauthMu.Lock()
		expectedState := a.oauthState
		verifier := a.oauthVerifier
		provider := a.oauthProvider
		redirectURI := a.oauthRedirectURI
		a.oauthState = ""
		a.oauthVerifier = ""
		a.oauthMu.Unlock()

		if r.URL.Query().Get("error") != "" {
			a.logf("oauth callback returned error: %s", r.URL.Query().Get("error"))
			http.Error(w, "The provider did not authorize the request.", http.StatusBadRequest)
			go server.Shutdown(context.Background())
			return
		}
		if expectedState == "" || r.URL.Query().Get("state") != expectedState {
			a.logf("oauth callback rejected: state mismatch")
			http.Error(w, "Authorization state did not match this sign-in.", http.StatusBadRequest)
			go server.Shutdown(context.Background())
			return
		}
		code := r.URL.Query().Get("code")
		if code == "" || verifier == "" {
			a.logf("oauth callback rejected: code_present=%t verifier_present=%t", code != "", verifier != "")
			http.Error(w, "Missing authorization code.", http.StatusBadRequest)
			go server.Shutdown(context.Background())
			return
		}
		w.Header().Set("Content-Type", "text/html; charset=utf-8")
		_, _ = io.WriteString(w, "<html><body style=\"font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Helvetica,Arial,sans-serif;text-align:center;padding-top:100px;background-color:#f8fafc;color:#1e293b;\"><h1>Authenticated</h1><p>You can close this tab and return to Meron.</p></body></html>")
		go server.Shutdown(context.Background())

		profile, err := exchangeOAuthCode(provider, code, verifier, redirectURI)
		if err != nil {
			a.logf("oauth exchange failed (provider=%s): %v", provider, err)
			fmt.Fprintf(os.Stderr, "meron: OAuth exchange failed: %v\n", err)
			return
		}
		profile.AuthCode = code
		a.oauthMu.Lock()
		a.oauthProfile = profile
		a.oauthMu.Unlock()
		a.logf("oauth callback exchanged profile for %s (provider=%s)", profile.Email, provider)
	})
	_ = server.Serve(listener)
}

// exchangeOAuthCode swaps an authorization code for tokens and resolves the
// account profile for the given provider. Gmail calls Google's userinfo
// endpoint; Outlook reads the OIDC id_token returned alongside the tokens (no
// extra request — Microsoft Graph is a separate resource the IMAP/SMTP token
// can't address).
func exchangeOAuthCode(provider, code, verifier, redirectURI string) (*ExchangedProfile, error) {
	switch provider {
	case "outlook":
		tokens, err := exchangeOutlookOAuthCodeWithVerifier(code, verifier, redirectURI)
		if err != nil {
			return nil, err
		}
		email, name := parseIDTokenClaims(tokens.IDToken)
		return &ExchangedProfile{
			Email:        email,
			DisplayName:  name,
			AccessToken:  tokens.AccessToken,
			RefreshToken: tokens.RefreshToken,
			ExpiresIn:    tokens.ExpiresIn,
		}, nil
	default: // gmail
		tokens, err := exchangeGmailOAuthCodeWithVerifier(code, verifier, redirectURI)
		if err != nil {
			return nil, err
		}
		user, err := fetchGoogleUserInfo(tokens.AccessToken)
		if err != nil {
			return nil, fmt.Errorf("Google userinfo failed: %w", err)
		}
		return &ExchangedProfile{
			Email:        user.Email,
			DisplayName:  user.Name,
			AvatarURL:    user.Picture,
			AccessToken:  tokens.AccessToken,
			RefreshToken: tokens.RefreshToken,
			ExpiresIn:    tokens.ExpiresIn,
		}, nil
	}
}

// exchangeOutlookOAuthCodeWithVerifier exchanges an authorization code at the
// Microsoft token endpoint. Public client (PKCE), so no client secret is sent.
func exchangeOutlookOAuthCodeWithVerifier(code, verifier, redirectURI string) (*TokenExchangeResult, error) {
	if verifier == "" {
		return nil, errors.New("Outlook OAuth code verifier missing")
	}
	values := url.Values{}
	values.Set("code", code)
	values.Set("client_id", outlookClientID())
	values.Set("code_verifier", verifier)
	values.Set("redirect_uri", redirectURI)
	values.Set("grant_type", "authorization_code")
	values.Set("scope", outlookScopes)
	res, err := http.PostForm(outlookTokenURL, values)
	if err != nil {
		return nil, err
	}
	defer res.Body.Close()
	body, _ := io.ReadAll(res.Body)
	if res.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("Outlook OAuth exchange failed: %s", strings.TrimSpace(string(body)))
	}
	var out TokenExchangeResult
	if err := json.Unmarshal(body, &out); err != nil {
		return nil, err
	}
	if out.RefreshToken == "" {
		return nil, errors.New("Outlook OAuth response did not include a refresh token")
	}
	return &out, nil
}

// parseIDTokenClaims decodes the (unverified) payload of an OIDC id_token to
// extract the account email and display name. The token was just received over
// TLS straight from Microsoft's token endpoint, so we read its claims without
// re-verifying the signature. Returns empty strings on any parse failure; the
// caller validates that an email is present.
func parseIDTokenClaims(idToken string) (email, name string) {
	parts := strings.Split(idToken, ".")
	if len(parts) != 3 {
		return "", ""
	}
	payload, err := base64.RawURLEncoding.DecodeString(parts[1])
	if err != nil {
		return "", ""
	}
	var claims struct {
		Email             string `json:"email"`
		PreferredUsername string `json:"preferred_username"`
		Name              string `json:"name"`
	}
	if err := json.Unmarshal(payload, &claims); err != nil {
		return "", ""
	}
	email = claims.Email
	if email == "" {
		email = claims.PreferredUsername
	}
	return email, claims.Name
}

func (a *App) exchangeGmailOAuthCode(code string) (*TokenExchangeResult, error) {
	a.oauthMu.Lock()
	verifier := a.oauthVerifier
	redirectURI := a.oauthRedirectURI
	a.oauthVerifier = ""
	a.oauthMu.Unlock()
	return exchangeGmailOAuthCodeWithVerifier(code, verifier, redirectURI)
}

func exchangeGmailOAuthCodeWithVerifier(code, verifier, redirectURI string) (*TokenExchangeResult, error) {
	if verifier == "" {
		return nil, errors.New("Google OAuth code verifier missing")
	}
	values := url.Values{}
	values.Set("code", code)
	values.Set("client_id", googleClientID())
	values.Set("client_secret", googleClientSecret())
	values.Set("code_verifier", verifier)
	values.Set("redirect_uri", redirectURI)
	values.Set("grant_type", "authorization_code")
	res, err := http.PostForm("https://oauth2.googleapis.com/token", values)
	if err != nil {
		return nil, err
	}
	defer res.Body.Close()
	body, _ := io.ReadAll(res.Body)
	if res.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("Google OAuth exchange failed: %s", strings.TrimSpace(string(body)))
	}
	var out TokenExchangeResult
	if err := json.Unmarshal(body, &out); err != nil {
		return nil, err
	}
	if out.RefreshToken == "" {
		return nil, errors.New("Google OAuth response did not include a refresh token")
	}
	return &out, nil
}

func (a *App) mailSync(payload map[string]any) (any, error) {
	accountID, _ := payload["account_id"].(string)
	folder, _ := payload["folder"].(string)
	if folder == "" {
		folder = "inbox"
	}
	engine := "meron_mail"
	params := map[string]any{"account": accountID, "folder": folder, "limit": 50}
	if isRSSAccountID(accountID) {
		engine = "rss"
		delete(params, "folder")
	}
	online := false
	if a.sidecar != nil && a.sidecar.Started() {
		// The sidecar routes messages.sync to the right engine by account.
		if _, err := a.sidecar.Call("messages.sync", params); err == nil {
			online = true
		}
	}
	return map[string]any{"ok": true, "synced_at": "now", "engine": engine, "online": online}, nil
}

func (a *App) watchFolder(command string, payload map[string]any) (any, error) {
	if a.sidecar == nil || !a.sidecar.Started() {
		return map[string]any{"ok": true, "online": false}, nil
	}
	return a.sidecar.Call(command, payload)
}

func (a *App) folderList(payload map[string]any) (any, error) {
	var req FolderListRequest
	_ = decode(payload, &req)
	if req.AccountID == "" {
		req.AccountID, _ = payload["account_id"].(string)
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return map[string]any{"folders": []Folder{}}, nil
	}
	res, err := a.sidecar.Call("folders.list", map[string]any{"account": req.AccountID, "refresh": req.Refresh})
	if err != nil {
		return nil, err
	}
	// RSS folders arrive already in final bridge shape; mail returns raw rows.
	if isRSSAccountID(req.AccountID) {
		return res, nil
	}
	return foldersJSON(req.AccountID, res), nil
}

func (a *App) folderCreate(payload map[string]any) (any, error) {
	var req FolderCreateRequest
	_ = decode(payload, &req)
	if req.AccountID == "" {
		req.AccountID, _ = payload["account_id"].(string)
	}
	if req.Name == "" {
		req.Name, _ = payload["name"].(string)
	}
	if req.AccountID == "" {
		return nil, errors.New("account_id is required")
	}
	if strings.TrimSpace(req.Name) == "" {
		return nil, errors.New("Folder name is required")
	}
	if isRSSAccountID(req.AccountID) {
		return nil, errors.New("RSS accounts do not support folders")
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	res, err := a.sidecar.Call("folders.create", map[string]any{"account": req.AccountID, "name": req.Name})
	if err != nil {
		return nil, err
	}
	return foldersJSON(req.AccountID, res), nil
}

func (a *App) suggestContacts(payload map[string]any) (any, error) {
	if a.sidecar == nil || !a.sidecar.Started() {
		return map[string]any{"contacts": []any{}}, nil
	}
	return a.sidecar.Call("contacts.suggest", payload)
}

func (a *App) threadList(payload map[string]any) (any, error) {
	var req ThreadListRequest
	_ = decode(payload, &req)
	if req.AccountID == "" {
		req.AccountID, _ = payload["account_id"].(string)
	}
	if req.FolderID == "" {
		req.FolderID = "inbox"
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return map[string]any{"threads": []Message{}}, nil
	}
	if req.Filter == "starred" {
		a.logf("threadList starred: account=%s folder=%s query=%q refresh=%t", req.AccountID, req.FolderID, req.Query, req.Refresh)
	}
	res, err := a.sidecar.Call("messages.recent", map[string]any{
		"account":       req.AccountID,
		"folder":        req.FolderID,
		"query":         req.Query,
		"filter":        req.Filter,
		"before_cursor": req.BeforeCursor,
		"limit":         50,
		"refresh":       req.Refresh,
	})
	if err != nil {
		return nil, err
	}
	// RSS threads arrive already in final bridge shape; mail returns raw rows.
	if isRSSAccountID(req.AccountID) {
		return res, nil
	}
	out := threadsJSON(req.AccountID, req.FolderID, res)
	if req.Filter == "starred" {
		rawCount := 0
		if obj, ok := res.(map[string]any); ok {
			if list, ok := obj["messages"].([]any); ok {
				rawCount = len(list)
			}
		}
		threadCount := 0
		if obj, ok := out.(map[string]any); ok {
			if list, ok := obj["threads"].([]Message); ok {
				threadCount = len(list)
			} else if list, ok := obj["threads"].([]any); ok {
				threadCount = len(list)
			}
		}
		a.logf("threadList starred: raw_messages=%d grouped_threads=%d", rawCount, threadCount)
	}
	return out, nil
}

// starredItems returns every starred item across all accounts as a flat list of
// per-message cards. Mail rows arrive raw and are shaped here (thread/message
// id minting lives in the bridge); RSS rows arrive final-shaped from the engine.
func (a *App) starredItems(payload map[string]any) (any, error) {
	if a.sidecar == nil || !a.sidecar.Started() {
		return map[string]any{"items": []Message{}}, nil
	}
	res, err := a.sidecar.Call("starred.items", payload)
	if err != nil {
		return nil, err
	}
	object, _ := res.(map[string]any)
	mailRows, _ := object["mail"].([]any)
	rssRows, _ := object["rss"].([]any)
	items := make([]any, 0, len(mailRows)+len(rssRows))
	for _, item := range mailRows {
		msg, _ := item.(map[string]any)
		uid := jsonNumber(msg["uid"])
		if uid <= 0 {
			continue
		}
		accountID := jsonString(msg["account"])
		folder := jsonString(msg["folder"])
		threadKey := jsonString(msg["thread_key"])
		if threadKey == "" {
			threadKey = fmt.Sprintf("uid:%d", uid)
		}
		compoundKey := threadKey
		if !strings.HasPrefix(threadKey, "uid:") {
			compoundKey = threadKey + "#" + threadGroupingSubject(jsonString(msg["subject"]))
		}
		// IDs must match threadsJSON/threadMessagesJSON exactly so opening the
		// thread and scrolling to the message line up with the conversation view.
		threadID := formatImapThreadID(accountID, folder, compoundKey)
		items = append(items, Message{
			ID:        fmt.Sprintf("%s#%d", threadID, uid),
			AccountID: accountID,
			FolderID:  folder,
			ThreadID:  threadID,
			FromName:  jsonString(msg["from_name"]),
			FromAddr:  jsonString(msg["from_addr"]),
			Subject:   jsonString(msg["subject"]),
			Date:      jsonNumber(msg["date"]),
			Unread:    !jsonBool(msg["seen"]),
			Starred:   true,
		})
	}
	items = append(items, rssRows...)
	return map[string]any{"items": items}, nil
}

func (a *App) threadRead(payload map[string]any) (any, error) {
	threadID, _ := payload["thread_id"].(string)
	if a.sidecar == nil || !a.sidecar.Started() {
		return map[string]any{"messages": []Message{}}, nil
	}
	// Pagination is opt-in from the frontend: when `limit` is omitted the
	// engine returns the full thread (used by markRead's scan path).
	limit, hasLimit := payload["limit"]
	beforeCursor, _ := payload["before_cursor"].(string)
	if _, _, ok := parseRSSThreadID(threadID); ok {
		params := map[string]any{"thread_id": threadID}
		if hasLimit {
			params["limit"] = limit
		}
		if beforeCursor != "" {
			params["before_cursor"] = beforeCursor
		}
		res, err := a.sidecar.Call("rss.thread", params)
		if err != nil {
			return nil, err
		}
		// Already final bridge-shaped Message JSON.
		return res, nil
	}
	if ids, ok := parseImapThreadID(threadID); ok {
		method := "messages.thread"
		realThreadKey := ids.ThreadKey
		subjectFilter := ""
		if strings.Contains(ids.ThreadKey, "#") {
			parts := strings.SplitN(ids.ThreadKey, "#", 2)
			realThreadKey = parts[0]
			subjectFilter = parts[1]
		}
		params := map[string]any{"account": ids.Account, "folder": ids.Folder, "thread_key": realThreadKey}
		if realThreadKey == "" && ids.UID > 0 {
			method = "messages.read"
			params = map[string]any{"account": ids.Account, "folder": ids.Folder, "uid": ids.UID}
		}
		if method == "messages.thread" {
			if hasLimit {
				params["limit"] = limit
			}
			if beforeCursor != "" {
				params["before_cursor"] = beforeCursor
			}
		}
		res, err := a.sidecar.Call(method, params)
		if err != nil {
			return nil, err
		}
		if method == "messages.thread" {
			return threadMessagesJSON(ids.Account, threadID, ids.Folder, res, subjectFilter), nil
		}
		return messageJSON(ids.Account, threadID, ids.Folder, res), nil
	}
	return map[string]any{"messages": []Message{}}, nil
}

func (a *App) mailDelete(payload map[string]any) (any, error) {
	threadID, _ := payload["thread_id"].(string)
	if threadID == "" {
		return map[string]any{"ok": true}, nil
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	if _, _, ok := parseRSSThreadID(threadID); ok {
		return nil, errors.New("feed items cannot be moved to trash")
	}
	if ids, ok := parseImapThreadID(threadID); ok {
		sourceFolder := deleteFolder(payload, ids.Folder)
		if strings.Contains(ids.ThreadKey, "#") {
			parts := strings.SplitN(ids.ThreadKey, "#", 2)
			realThreadKey := parts[0]
			subjectFilter := parts[1]
			if uids := imapUIDsFromPayload(threadID, payload); len(uids) > 0 {
				res, err := a.sidecar.Call("messages.delete", map[string]any{"account": ids.Account, "folder": sourceFolder, "uids": uids})
				if err != nil {
					return nil, err
				}
				return withMovedThreadLocation(res, ids, "trash"), nil
			}
			res, err := a.sidecar.Call("messages.threadHeaders", map[string]any{
				"account":    ids.Account,
				"folder":     sourceFolder,
				"thread_key": realThreadKey,
			})
			if err != nil {
				return nil, err
			}
			uidsByFolder := uidsByFolderForSubjectThread(res, sourceFolder, subjectFilter)
			if len(uidsByFolder) == 0 {
				return map[string]any{"ok": true, "deleted": 0}, nil
			}
			deleted := 0
			trashFolder := ""
			permanent := false
			for msgFolder, uids := range uidsByFolder {
				res, err := a.sidecar.Call("messages.delete", map[string]any{"account": ids.Account, "folder": msgFolder, "uids": uids})
				if err != nil {
					return nil, err
				}
				if obj, ok := res.(map[string]any); ok {
					deleted += int(jsonNumber(obj["deleted"]))
					if trash := jsonString(obj["trash"]); trash != "" {
						trashFolder = trash
					}
					if jsonBool(obj["permanent"]) {
						permanent = true
					}
				}
			}
			out := map[string]any{"ok": true, "deleted": deleted}
			if trashFolder != "" {
				out["trash"] = trashFolder
				out["thread_id"] = formatParsedImapThreadIDInFolder(ids, trashFolder)
			}
			if permanent {
				out["permanent"] = true
			}
			return out, nil
		}
		params := map[string]any{"account": ids.Account, "folder": sourceFolder, "thread_key": ids.ThreadKey}
		if uids := imapUIDsFromPayload(threadID, payload); len(uids) > 0 {
			params["uids"] = uids
			delete(params, "thread_key")
			res, err := a.sidecar.Call("messages.delete", params)
			if err != nil {
				return nil, err
			}
			return withMovedThreadLocation(res, ids, "trash"), nil
		}
		if ids.ThreadKey == "" && ids.UID > 0 {
			params = map[string]any{"account": ids.Account, "folder": sourceFolder, "uid": ids.UID}
		}
		res, err := a.sidecar.Call("messages.delete", params)
		if err != nil {
			return nil, err
		}
		return withMovedThreadLocation(res, ids, "trash"), nil
	}
	return map[string]any{"ok": true, "deleted": 0}, nil
}

func (a *App) mailMove(payload map[string]any) (any, error) {
	threadID, _ := payload["thread_id"].(string)
	targetFolder, _ := payload["target_folder_id"].(string)
	if threadID == "" || targetFolder == "" {
		return map[string]any{"ok": true}, nil
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	if _, _, ok := parseRSSThreadID(threadID); ok {
		return nil, errors.New("feed items cannot be moved between folders")
	}
	if ids, ok := parseImapThreadID(threadID); ok {
		if canonThreadFolder(ids.Folder) == canonThreadFolder(targetFolder) {
			return map[string]any{"ok": true, "moved": 0}, nil
		}
		if strings.Contains(ids.ThreadKey, "#") {
			parts := strings.SplitN(ids.ThreadKey, "#", 2)
			realThreadKey := parts[0]
			subjectFilter := parts[1]
			if uids := imapUIDsFromPayload(threadID, payload); len(uids) > 0 {
				return a.sidecar.Call("messages.move", map[string]any{"account": ids.Account, "folder": ids.Folder, "target_folder": targetFolder, "uids": uids})
			}
			res, err := a.sidecar.Call("messages.threadHeaders", map[string]any{
				"account":    ids.Account,
				"folder":     ids.Folder,
				"thread_key": realThreadKey,
			})
			if err != nil {
				return nil, err
			}
			uidsByFolder := uidsByFolderForSubjectThread(res, ids.Folder, subjectFilter)
			moved := 0
			for msgFolder, uids := range uidsByFolder {
				if msgFolder == targetFolder || len(uids) == 0 {
					continue
				}
				res, err := a.sidecar.Call("messages.move", map[string]any{"account": ids.Account, "folder": msgFolder, "target_folder": targetFolder, "uids": uids})
				if err != nil {
					return nil, err
				}
				if obj, ok := res.(map[string]any); ok {
					moved += int(jsonNumber(obj["moved"]))
				}
			}
			return map[string]any{"ok": true, "moved": moved}, nil
		}
		params := map[string]any{"account": ids.Account, "folder": ids.Folder, "target_folder": targetFolder, "thread_key": ids.ThreadKey}
		if uids := imapUIDsFromPayload(threadID, payload); len(uids) > 0 {
			params["uids"] = uids
			delete(params, "thread_key")
			return a.sidecar.Call("messages.move", params)
		}
		if ids.ThreadKey == "" && ids.UID > 0 {
			params = map[string]any{"account": ids.Account, "folder": ids.Folder, "target_folder": targetFolder, "uid": ids.UID}
		}
		return a.sidecar.Call("messages.move", params)
	}
	return map[string]any{"ok": true}, nil
}

func (a *App) mailCopy(payload map[string]any) (any, error) {
	threadID, _ := payload["thread_id"].(string)
	targetAccount, _ := payload["target_account_id"].(string)
	targetFolder, _ := payload["target_folder_id"].(string)
	if threadID == "" || targetAccount == "" || targetFolder == "" {
		return map[string]any{"ok": true}, nil
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	if _, _, ok := parseRSSThreadID(threadID); ok {
		return nil, errors.New("feed items cannot be copied between folders")
	}
	if ids, ok := parseImapThreadID(threadID); ok {
		if strings.Contains(ids.ThreadKey, "#") {
			parts := strings.SplitN(ids.ThreadKey, "#", 2)
			realThreadKey := parts[0]
			subjectFilter := parts[1]
			if uids := imapUIDsFromPayload(threadID, payload); len(uids) > 0 {
				return a.sidecar.Call("messages.copy", map[string]any{
					"account":        ids.Account,
					"folder":         ids.Folder,
					"target_account": targetAccount,
					"target_folder":  targetFolder,
					"uids":           uids,
				})
			}
			res, err := a.sidecar.Call("messages.threadHeaders", map[string]any{
				"account":    ids.Account,
				"folder":     ids.Folder,
				"thread_key": realThreadKey,
			})
			if err != nil {
				return nil, err
			}
			uidsByFolder := uidsByFolderForSubjectThread(res, ids.Folder, subjectFilter)
			copied := 0
			for msgFolder, uids := range uidsByFolder {
				if len(uids) == 0 {
					continue
				}
				res, err := a.sidecar.Call("messages.copy", map[string]any{
					"account":        ids.Account,
					"folder":         msgFolder,
					"target_account": targetAccount,
					"target_folder":  targetFolder,
					"uids":           uids,
				})
				if err != nil {
					return nil, err
				}
				if obj, ok := res.(map[string]any); ok {
					copied += int(jsonNumber(obj["copied"]))
				}
			}
			return map[string]any{
				"ok":               true,
				"copied":           copied,
				"target_account":   targetAccount,
				"target_folder":    targetFolder,
				"target_thread_id": formatImapThreadID(targetAccount, targetFolder, ids.ThreadKey),
			}, nil
		}
		params := map[string]any{
			"account":        ids.Account,
			"folder":         ids.Folder,
			"target_account": targetAccount,
			"target_folder":  targetFolder,
			"thread_key":     ids.ThreadKey,
		}
		if uids := imapUIDsFromPayload(threadID, payload); len(uids) > 0 {
			params["uids"] = uids
			delete(params, "thread_key")
			return a.sidecar.Call("messages.copy", params)
		}
		if ids.ThreadKey == "" && ids.UID > 0 {
			params = map[string]any{
				"account":        ids.Account,
				"folder":         ids.Folder,
				"target_account": targetAccount,
				"target_folder":  targetFolder,
				"uid":            ids.UID,
			}
		}
		res, err := a.sidecar.Call("messages.copy", params)
		if err != nil {
			return nil, err
		}
		out, _ := res.(map[string]any)
		if out == nil {
			out = map[string]any{"ok": true}
		}
		if ids.ThreadKey != "" {
			out["target_thread_id"] = formatImapThreadID(targetAccount, targetFolder, ids.ThreadKey)
		}
		return out, nil
	}
	return map[string]any{"ok": true}, nil
}

func uidsByFolderForSubjectThread(res any, fallbackFolder, subjectFilter string) map[string][]uint32 {
	uidsByFolder := make(map[string][]uint32)
	if obj, ok := res.(map[string]any); ok {
		if list, ok := obj["headers"].([]any); ok {
			for _, item := range list {
				entry, _ := item.(map[string]any)
				normSub := threadGroupingSubject(jsonString(entry["subject"]))
				if normSub == subjectFilter {
					uid := uint32(jsonNumber(entry["uid"]))
					if uid > 0 {
						msgFolder := jsonString(entry["folder"])
						if msgFolder == "" {
							msgFolder = fallbackFolder
						}
						uidsByFolder[msgFolder] = append(uidsByFolder[msgFolder], uid)
					}
				}
			}
		}
		if list, ok := obj["messages"].([]any); ok {
			for _, item := range list {
				entry, _ := item.(map[string]any)
				msg, _ := entry["message"].(map[string]any)
				normSub := threadGroupingSubject(jsonString(msg["subject"]))
				if normSub == subjectFilter {
					uid := uint32(jsonNumber(entry["uid"]))
					if uid > 0 {
						msgFolder := jsonString(entry["folder"])
						if msgFolder == "" {
							msgFolder = fallbackFolder
						}
						uidsByFolder[msgFolder] = append(uidsByFolder[msgFolder], uid)
					}
				}
			}
		}
	}
	return uidsByFolder
}

func (a *App) mailArchive(payload map[string]any) (any, error) {
	threadID, _ := payload["thread_id"].(string)
	if threadID == "" {
		return map[string]any{"ok": true}, nil
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	if _, _, ok := parseRSSThreadID(threadID); ok {
		return nil, errors.New("feed items cannot be archived")
	}
	ids, ok := parseImapThreadID(threadID)
	if !ok {
		return map[string]any{"ok": true}, nil
	}
	res, err := a.sidecar.Call("folders.archive", map[string]any{"account": ids.Account})
	if err != nil {
		return nil, err
	}
	obj, _ := res.(map[string]any)
	targetFolder, _ := obj["folder"].(string)
	if targetFolder == "" {
		return nil, errors.New("Archive folder not found for this account")
	}
	payload["target_folder_id"] = targetFolder
	moveRes, err := a.mailMove(payload)
	if err != nil {
		return nil, err
	}
	out, _ := moveRes.(map[string]any)
	if out == nil {
		out = map[string]any{"ok": true}
	}
	out["folder"] = targetFolder
	out["thread_id"] = formatParsedImapThreadIDInFolder(ids, targetFolder)
	return out, nil
}

func (a *App) markRead(payload map[string]any) (any, error) {
	threadID, _ := payload["thread_id"].(string)
	if threadID == "" {
		return map[string]any{"ok": true}, nil
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return map[string]any{"ok": true}, nil
	}
	// Defaults to read; pass seen:false to mark a thread unread.
	seen := true
	if v, ok := payload["seen"].(bool); ok {
		seen = v
	}
	if _, _, ok := parseRSSThreadID(threadID); ok {
		if itemKeys := rssItemKeysFromPayload(threadID, payload); len(itemKeys) > 0 {
			return a.sidecar.Call("rss.markRead", map[string]any{"thread_id": threadID, "item_keys": itemKeys, "seen": seen})
		}
		return a.sidecar.Call("rss.markRead", map[string]any{"thread_id": threadID, "seen": seen})
	}
	if ids, ok := parseImapThreadID(threadID); ok {
		if strings.Contains(ids.ThreadKey, "#") {
			parts := strings.SplitN(ids.ThreadKey, "#", 2)
			realThreadKey := parts[0]
			subjectFilter := parts[1]
			if uids := imapUIDsFromPayload(threadID, payload); len(uids) > 0 {
				params := map[string]any{"account": ids.Account, "folder": ids.Folder, "uids": uids, "seen": seen}
				return a.sidecar.Call("messages.markRead", params)
			}
			res, err := a.sidecar.Call("messages.thread", map[string]any{
				"account":    ids.Account,
				"folder":     ids.Folder,
				"thread_key": realThreadKey,
			})
			if err != nil {
				return nil, err
			}
			var uids []uint32
			if obj, ok := res.(map[string]any); ok {
				if list, ok := obj["messages"].([]any); ok {
					for _, item := range list {
						entry, _ := item.(map[string]any)
						// Only touch messages whose flag differs from the target.
						if jsonBool(entry["seen"]) == seen {
							continue
						}
						msg, _ := entry["message"].(map[string]any)
						normSub := threadGroupingSubject(jsonString(msg["subject"]))
						if normSub == subjectFilter {
							uid := uint32(jsonNumber(entry["uid"]))
							if uid > 0 {
								uids = append(uids, uid)
							}
						}
					}
				}
			}
			if len(uids) == 0 {
				return map[string]any{"ok": true}, nil
			}
			params := map[string]any{
				"account": ids.Account,
				"folder":  ids.Folder,
				"uids":    uids,
				"seen":    seen,
			}
			return a.sidecar.Call("messages.markRead", params)
		}
		params := map[string]any{"account": ids.Account, "folder": ids.Folder, "thread_key": ids.ThreadKey, "seen": seen}
		if uids := imapUIDsFromPayload(threadID, payload); len(uids) > 0 {
			params["uids"] = uids
			delete(params, "thread_key")
			return a.sidecar.Call("messages.markRead", params)
		}
		if ids.ThreadKey == "" && ids.UID > 0 {
			params = map[string]any{"account": ids.Account, "folder": ids.Folder, "uid": ids.UID, "seen": seen}
		}
		return a.sidecar.Call("messages.markRead", params)
	}
	return map[string]any{"ok": true}, nil
}

func (a *App) markStarred(payload map[string]any) (any, error) {
	threadID, _ := payload["thread_id"].(string)
	if threadID == "" {
		return map[string]any{"ok": true}, nil
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return map[string]any{"ok": true}, nil
	}
	starred := true
	if v, ok := payload["starred"].(bool); ok {
		starred = v
	}
	if _, _, ok := parseRSSThreadID(threadID); ok {
		if itemKeys := rssItemKeysFromPayload(threadID, payload); len(itemKeys) > 0 {
			return a.sidecar.Call("rss.markStarred", map[string]any{"thread_id": threadID, "item_keys": itemKeys, "starred": starred})
		}
		return a.sidecar.Call("rss.markStarred", map[string]any{"thread_id": threadID, "starred": starred})
	}
	if ids, ok := parseImapThreadID(threadID); ok {
		if strings.Contains(ids.ThreadKey, "#") {
			parts := strings.SplitN(ids.ThreadKey, "#", 2)
			realThreadKey := parts[0]
			subjectFilter := parts[1]
			if uids := imapUIDsFromPayload(threadID, payload); len(uids) > 0 {
				params := map[string]any{"account": ids.Account, "folder": ids.Folder, "uids": uids, "starred": starred}
				return a.sidecar.Call("messages.markStarred", params)
			}
			res, err := a.sidecar.Call("messages.thread", map[string]any{
				"account":    ids.Account,
				"folder":     ids.Folder,
				"thread_key": realThreadKey,
			})
			if err != nil {
				return nil, err
			}
			uidsByFolder := make(map[string][]uint32)
			if obj, ok := res.(map[string]any); ok {
				if list, ok := obj["messages"].([]any); ok {
					for _, item := range list {
						entry, _ := item.(map[string]any)
						if jsonBool(entry["starred"]) == starred {
							continue
						}
						msg, _ := entry["message"].(map[string]any)
						normSub := threadGroupingSubject(jsonString(msg["subject"]))
						if normSub == subjectFilter {
							uid := uint32(jsonNumber(entry["uid"]))
							if uid > 0 {
								msgFolder := jsonString(entry["folder"])
								if msgFolder == "" {
									msgFolder = ids.Folder
								}
								uidsByFolder[msgFolder] = append(uidsByFolder[msgFolder], uid)
							}
						}
					}
				}
			}
			if len(uidsByFolder) == 0 {
				return map[string]any{"ok": true}, nil
			}
			for msgFolder, uids := range uidsByFolder {
				params := map[string]any{
					"account": ids.Account,
					"folder":  msgFolder,
					"uids":    uids,
					"starred": starred,
				}
				if _, err := a.sidecar.Call("messages.markStarred", params); err != nil {
					return nil, err
				}
			}
			return map[string]any{"ok": true}, nil
		}
		params := map[string]any{"account": ids.Account, "folder": ids.Folder, "thread_key": ids.ThreadKey, "starred": starred}
		if uids := imapUIDsFromPayload(threadID, payload); len(uids) > 0 {
			params["uids"] = uids
			delete(params, "thread_key")
			return a.sidecar.Call("messages.markStarred", params)
		}
		if ids.ThreadKey == "" && ids.UID > 0 {
			params = map[string]any{"account": ids.Account, "folder": ids.Folder, "uid": ids.UID, "starred": starred}
		}
		return a.sidecar.Call("messages.markStarred", params)
	}
	return map[string]any{"ok": true}, nil
}

// markAllRead marks every message in one mail folder as read. RSS accounts have
// no folder-wide flag, so the frontend marks those threads individually and this
// path is a no-op for them.
func (a *App) markAllRead(payload map[string]any) (any, error) {
	accountID, _ := payload["account_id"].(string)
	folderID, _ := payload["folder_id"].(string)
	if folderID == "" {
		folderID = "inbox"
	}
	if a.sidecar == nil || !a.sidecar.Started() || accountID == "" {
		return map[string]any{"ok": true}, nil
	}
	if isRSSAccountID(accountID) {
		return map[string]any{"ok": true}, nil
	}
	return a.sidecar.Call("messages.markAllRead", map[string]any{
		"account": accountID,
		"folder":  folderID,
	})
}

// saveAttachment copies an attachment the sidecar already wrote under the media
// dir (served at /media/<key>) to a user-chosen path via a native save dialog.
// The key is path-cleaned and confined to the media root to block traversal.
func (a *App) saveAttachment(payload map[string]any) (any, error) {
	key, _ := payload["key"].(string)
	filename, _ := payload["filename"].(string)
	if key == "" {
		return nil, errors.New("missing attachment key")
	}

	root := mediaDir()
	// Cleaning a rooted "/"+key strips any "..", then Join confines it to root.
	src := filepath.Join(root, filepath.Clean("/"+key))
	if rel, err := filepath.Rel(root, src); err != nil || rel == ".." || strings.HasPrefix(rel, ".."+string(os.PathSeparator)) {
		return nil, errors.New("invalid attachment key")
	}
	if filename == "" {
		filename = filepath.Base(src)
	}

	dest, err := wailsRuntime.SaveFileDialog(a.ctx, wailsRuntime.SaveDialogOptions{
		Title:                "Save attachment",
		DefaultFilename:      filename,
		CanCreateDirectories: true,
	})
	if err != nil {
		return nil, err
	}
	if dest == "" {
		return map[string]any{"saved": false}, nil // user cancelled
	}

	in, err := os.Open(src)
	if err != nil {
		return nil, fmt.Errorf("open attachment: %w", err)
	}
	defer in.Close()
	out, err := os.Create(dest)
	if err != nil {
		return nil, fmt.Errorf("create destination: %w", err)
	}
	if _, err := io.Copy(out, in); err != nil {
		out.Close()
		return nil, fmt.Errorf("write attachment: %w", err)
	}
	if err := out.Close(); err != nil {
		return nil, fmt.Errorf("write attachment: %w", err)
	}
	return map[string]any{"saved": true, "path": dest}, nil
}

// readAttachment reads an attachment the sidecar already wrote under the media
// dir (served at /media/<key>) and returns it base64-encoded, for pulling a
// stored attachment back into the composer (e.g. "Edit as New Message"). The key
// is path-cleaned and confined to the media root to block traversal.
func (a *App) readAttachment(payload map[string]any) (any, error) {
	key, _ := payload["key"].(string)
	if key == "" {
		return nil, errors.New("missing attachment key")
	}

	root := mediaDir()
	// Cleaning a rooted "/"+key strips any "..", then Join confines it to root.
	src := filepath.Join(root, filepath.Clean("/"+key))
	if rel, err := filepath.Rel(root, src); err != nil || rel == ".." || strings.HasPrefix(rel, ".."+string(os.PathSeparator)) {
		return nil, errors.New("invalid attachment key")
	}

	data, err := os.ReadFile(src)
	if err != nil {
		return nil, fmt.Errorf("read attachment: %w", err)
	}
	mime := http.DetectContentType(data)
	return map[string]any{
		"data": base64.StdEncoding.EncodeToString(data),
		"mime": mime,
		"size": len(data),
	}, nil
}

func rssItemKeysFromPayload(threadID string, payload map[string]any) []string {
	raw, _ := payload["message_ids"].([]any)
	itemKeys := make([]string, 0, len(raw))
	prefix := threadID + "#"
	for _, item := range raw {
		messageID, _ := item.(string)
		itemKey, ok := strings.CutPrefix(messageID, prefix)
		if ok && itemKey != "" {
			itemKeys = append(itemKeys, itemKey)
		}
	}
	return itemKeys
}

// deleteFolder lets delete callers override the thread id's nominal folder with
// the message/thread row's current folder. Thread ids can outlive a move, and a
// UID/thread key is only meaningful within its source mailbox.
func deleteFolder(payload map[string]any, fallback string) string {
	if folder, ok := payload["folder"].(string); ok && folder != "" {
		return folder
	}
	return fallback
}

func imapUIDsFromPayload(threadID string, payload map[string]any) []uint32 {
	raw, _ := payload["message_ids"].([]any)
	uids := make([]uint32, 0, len(raw))
	prefix := threadID + "#"
	for _, item := range raw {
		messageID, _ := item.(string)
		uidPart := messageID
		if rest, ok := strings.CutPrefix(messageID, prefix); ok {
			uidPart = rest
		}
		uid, err := strconv.ParseUint(uidPart, 10, 32)
		if err == nil {
			uids = append(uids, uint32(uid))
			continue
		}
		if ids, ok := parseImapThreadID(messageID); ok && ids.UID > 0 {
			uids = append(uids, ids.UID)
		}
	}
	return uids
}

func (a *App) mailSend(payload map[string]any) (any, error) {
	var req SendMailRequest
	if err := decode(payload, &req); err != nil {
		return nil, err
	}
	if req.To == "" {
		return nil, errors.New("invalid message")
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	if _, err := a.sidecar.Call("send", map[string]any{
		"account":     req.AccountID,
		"to":          req.To,
		"cc":          req.Cc,
		"bcc":         req.Bcc,
		"subject":     req.Subject,
		"body":        req.Body,
		"html":        req.Html,
		"in_reply_to": req.InReplyTo,
		"references":  req.References,
		"reply_to":    req.ReplyTo,
		"from":        req.From,
		"message_id":  req.MessageID,
		"attachments": req.Attachments,
	}); err != nil {
		return nil, err
	}
	return map[string]any{"ok": true, "queued": false}, nil
}

func (a *App) mailSaveDraft(payload map[string]any) (any, error) {
	var req SendMailRequest
	if err := decode(payload, &req); err != nil {
		return nil, err
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	if _, err := a.sidecar.Call("save_draft", map[string]any{
		"account":     req.AccountID,
		"to":          req.To,
		"cc":          req.Cc,
		"bcc":         req.Bcc,
		"subject":     req.Subject,
		"body":        req.Body,
		"html":        req.Html,
		"in_reply_to": req.InReplyTo,
		"references":  req.References,
		"reply_to":    req.ReplyTo,
		"from":        req.From,
		"draft_id":    req.DraftID,
		"attachments": req.Attachments,
	}); err != nil {
		return nil, err
	}
	return map[string]any{"ok": true}, nil
}

func (a *App) mailDiscardDraft(payload map[string]any) (any, error) {
	accountID, _ := payload["account_id"].(string)
	draftID, _ := payload["draft_id"].(string)
	if accountID == "" || draftID == "" {
		return map[string]any{"ok": true, "deleted": 0}, nil
	}
	if a.sidecar == nil || !a.sidecar.Started() {
		return nil, errors.New("mail engine unavailable")
	}
	return a.sidecar.Call("discard_draft", map[string]any{
		"account":  accountID,
		"draft_id": draftID,
	})
}

type Sidecar struct {
	path    string
	cmd     *exec.Cmd
	stdin   io.WriteCloser
	started bool
	mu      sync.Mutex
	nextID  uint64
	pending map[uint64]chan sidecarResponse
	cancel  context.CancelFunc
	stderr  io.Writer
	onEvent func(name string, detail any)
}

type sidecarResponse struct {
	Result any
	Error  any
}

func NewSidecar(path string, stderr io.Writer) *Sidecar {
	if stderr == nil {
		stderr = os.Stderr
	}
	return &Sidecar{path: path, stderr: stderr, pending: map[uint64]chan sidecarResponse{}}
}

func (s *Sidecar) Started() bool {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.started
}

func (s *Sidecar) Start(ctx context.Context) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if s.started {
		return nil
	}
	runCtx, cancel := context.WithCancel(context.Background())
	cmd := exec.CommandContext(runCtx, s.path)
	cmd.Env = sidecarEnv()
	cmd.Stderr = s.stderr
	stdin, err := cmd.StdinPipe()
	if err != nil {
		cancel()
		return err
	}
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		cancel()
		return err
	}
	if err := cmd.Start(); err != nil {
		cancel()
		return err
	}
	s.cmd = cmd
	s.stdin = stdin
	s.cancel = cancel
	s.started = true
	go s.readLoop(ctx, stdout)
	go func() {
		_ = cmd.Wait()
		s.mu.Lock()
		s.started = false
		for id, ch := range s.pending {
			delete(s.pending, id)
			ch <- sidecarResponse{Error: "sidecar exited"}
		}
		s.mu.Unlock()
	}()
	return nil
}

func sidecarEnv() []string {
	// Pass storage/media paths explicitly so the Go bridge and Rust sidecar use
	// the same dev or production profile.
	env := append(os.Environ(),
		"MERON_CORE_DB="+filepath.Join(appConfigDir(), "meron.db"),
		"MERON_MEDIA_DIR="+mediaDir(),
	)
	// Pass the resolved Google credentials down so the sidecar's OAuth refresh
	// path can read them from the environment without its own deobfuscation.
	// In release builds these come from the baked-in obfuscated defaults; in
	// dev they come from the MERON_GOOGLE_* env vars (which take priority).
	if id := googleClientID(); id != "" {
		env = append(env, "MERON_GOOGLE_CLIENT_ID="+id)
	}
	if secret := googleClientSecret(); secret != "" {
		env = append(env, "MERON_GOOGLE_CLIENT_SECRET="+secret)
	}
	if id := outlookClientID(); id != "" {
		env = append(env, "MERON_OUTLOOK_CLIENT_ID="+id)
	}
	return env
}

func (s *Sidecar) Close() {
	s.mu.Lock()
	cancel := s.cancel
	cmd := s.cmd
	s.started = false
	s.mu.Unlock()
	if cancel != nil {
		cancel()
	}
	if cmd != nil && cmd.Process != nil {
		_ = cmd.Process.Kill()
	}
}

func (s *Sidecar) Call(method string, params any) (any, error) {
	timeout := sidecarCallTimeout(method)
	s.mu.Lock()
	if !s.started {
		s.mu.Unlock()
		return nil, errors.New("sidecar not started")
	}
	s.nextID++
	id := s.nextID
	ch := make(chan sidecarResponse, 1)
	s.pending[id] = ch
	req := map[string]any{"id": id, "method": method, "params": params}
	line, err := json.Marshal(req)
	if err != nil {
		delete(s.pending, id)
		s.mu.Unlock()
		return nil, err
	}
	line = append(line, '\n')
	start := time.Now()
	_, err = s.stdin.Write(line)
	s.mu.Unlock()
	if err != nil {
		return nil, err
	}
	select {
	case res := <-ch:
		if res.Error != nil {
			return nil, fmt.Errorf("sidecar %s error: %s", method, sidecarErrorMessage(res.Error))
		}
		return res.Result, nil
	case <-time.After(timeout):
		s.mu.Lock()
		delete(s.pending, id)
		s.mu.Unlock()
		return nil, fmt.Errorf("sidecar %s timeout after %s", method, time.Since(start).Round(time.Millisecond))
	}
}

func (s *Sidecar) readLoop(ctx context.Context, stdout io.Reader) {
	scanner := bufio.NewScanner(stdout)
	scanner.Buffer(make([]byte, 0, 64*1024), 16*1024*1024)
	for scanner.Scan() {
		var object map[string]any
		if err := json.Unmarshal(scanner.Bytes(), &object); err != nil {
			continue
		}
		if eventName, ok := object["event"].(string); ok {
			if ctx != nil {
				wailsRuntime.EventsEmit(ctx, eventName, object["detail"])
			}
			if s.onEvent != nil {
				s.onEvent(eventName, object["detail"])
			}
			continue
		}
		idFloat, ok := object["id"].(float64)
		if !ok {
			continue
		}
		id := uint64(idFloat)
		s.mu.Lock()
		ch := s.pending[id]
		delete(s.pending, id)
		s.mu.Unlock()
		if ch == nil {
			continue
		}
		if errValue, ok := object["error"]; ok {
			ch <- sidecarResponse{Error: errValue}
		} else {
			ch <- sidecarResponse{Result: object["result"]}
		}
	}
}

// accountList returns every account (mail + rss) from the sidecar's unified DB,
// the single source of truth.
func (a *App) accountList() (any, error) {
	if a.sidecar == nil || !a.sidecar.Started() {
		return map[string]any{"accounts": []Account{}}, nil
	}
	res, err := a.sidecar.Call("account.list", map[string]any{})
	if err != nil {
		return nil, err
	}
	return map[string]any{"accounts": decodeAccounts(res)}, nil
}

// isRSSAccountID reports whether an account id belongs to the RSS engine. RSS ids
// are minted with an "rss-" prefix (see the sidecar's rss module), so routing
// stays stateless — no per-call engine lookup in the bridge.
func isRSSAccountID(id string) bool {
	return strings.HasPrefix(id, "rss-")
}

// parseRSSThreadID splits an RSS thread id ("<account>#rss#<sub>[#item]").
func parseRSSThreadID(threadID string) (string, string, bool) {
	accountID, rest, ok := strings.Cut(threadID, "#rss#")
	if !ok || accountID == "" || rest == "" {
		return "", "", false
	}
	subID, _, _ := strings.Cut(rest, "#")
	if subID == "" {
		return "", "", false
	}
	return accountID, subID, true
}

func decodeAccount(raw any) Account {
	obj, _ := raw.(map[string]any)
	return accountFromMap(obj["account"])
}

func decodeAccounts(raw any) []Account {
	obj, _ := raw.(map[string]any)
	list, _ := obj["accounts"].([]any)
	out := make([]Account, 0, len(list))
	for _, item := range list {
		out = append(out, accountFromMap(item))
	}
	return out
}

func accountFromMap(item any) Account {
	var acc Account
	data, _ := json.Marshal(item)
	_ = json.Unmarshal(data, &acc)
	return acc
}

type Account struct {
	ID                string  `json:"id"`
	Email             string  `json:"email"`
	DisplayName       string  `json:"display_name"`
	SenderName        string  `json:"sender_name"`
	AvatarURL         string  `json:"avatar_url,omitempty"`
	Provider          string  `json:"provider"`
	AuthType          string  `json:"auth_type"`
	IMAPHost          string  `json:"imap_host"`
	IMAPPort          uint16  `json:"imap_port"`
	SMTPHost          string  `json:"smtp_host"`
	SMTPPort          uint16  `json:"smtp_port"`
	TLS               bool    `json:"tls"`
	LoadRemoteImages  bool    `json:"load_remote_images"`
	ConversationHTML  bool    `json:"conversation_html"`
	ChatWallpaper     any     `json:"chat_wallpaper,omitempty"`
	IncludedInUnified bool    `json:"included_in_unified"`
	Muted             bool    `json:"muted"`
	Paused            bool    `json:"paused"`
	NeedsReconnect    bool    `json:"needs_reconnect,omitempty"`
	FeedURL           string  `json:"feed_url,omitempty"`
	AccessToken       string  `json:"access_token,omitempty"`
	RefreshToken      string  `json:"refresh_token,omitempty"`
	TokenExpiresAt    int64   `json:"token_expires_at,omitempty"`
	SortOrder         int     `json:"sort_order"`
	Aliases           []Alias `json:"aliases,omitempty"`
}

// Alias is a send-as identity for an account: an address the user owns plus an
// optional From display name (blank falls back to the account's sender name).
type Alias struct {
	Email string `json:"email"`
	Name  string `json:"name,omitempty"`
}

type Folder struct {
	ID        string `json:"id"`
	AccountID string `json:"account_id"`
	Name      string `json:"name"`
	Role      string `json:"role"`
	Delimiter string `json:"delimiter"`
	Unread    uint32 `json:"unread"`
}

type Message struct {
	ID               string `json:"id"`
	AccountID        string `json:"account_id"`
	FolderID         string `json:"folder_id"`
	ThreadID         string `json:"thread_id"`
	FromName         string `json:"from_name"`
	FromAddr         string `json:"from_addr"`
	To               string `json:"to"`
	ReplyTo          string `json:"reply_to,omitempty"`
	Cc               string `json:"cc,omitempty"`
	Bcc              string `json:"bcc,omitempty"`
	MessageID        string `json:"message_id,omitempty"`
	References       string `json:"references,omitempty"`
	Subject          string `json:"subject"`
	Preview          string `json:"preview"`
	Body             string `json:"body"`
	BodyHTML         string `json:"body_html,omitempty"`
	Date             int64  `json:"date"`
	Unread           bool   `json:"unread"`
	UnreadCount      uint32 `json:"unread_count,omitempty"`
	Starred          bool   `json:"starred"`
	HasAttachments   bool   `json:"has_attachments"`
	Attachments      any    `json:"attachments,omitempty"`
	OriginalThreadID string `json:"original_thread_id,omitempty"`
	// RecipientOverflow is the count of additional recipients beyond the one shown
	// on an outbound thread card (for a "+N" hint); 0 for inbound/single-recipient.
	RecipientOverflow uint32 `json:"recipient_overflow,omitempty"`
}

type AddPasswordAccountRequest struct {
	Email       string `json:"email"`
	DisplayName string `json:"display_name"`
	SenderName  string `json:"sender_name"`
	IMAPHost    string `json:"imap_host"`
	IMAPPort    uint16 `json:"imap_port"`
	SMTPHost    string `json:"smtp_host"`
	SMTPPort    uint16 `json:"smtp_port"`
	Username    string `json:"username"`
	Password    string `json:"password"`
	TLS         bool   `json:"tls"`
}

type AddGmailOAuthRequest struct {
	Email        string `json:"email"`
	DisplayName  string `json:"display_name"`
	SenderName   string `json:"sender_name"`
	AvatarURL    string `json:"avatar_url"`
	AuthCode     string `json:"auth_code"`
	AccessToken  string `json:"access_token"`
	RefreshToken string `json:"refresh_token"`
	ExpiresIn    int64  `json:"expires_in"`
}

// AddOutlookOAuthRequest mirrors AddGmailOAuthRequest. AvatarURL is unused
// (Microsoft's id_token carries no picture) but kept for shape parity.
type AddOutlookOAuthRequest struct {
	Email        string `json:"email"`
	DisplayName  string `json:"display_name"`
	SenderName   string `json:"sender_name"`
	AvatarURL    string `json:"avatar_url"`
	AuthCode     string `json:"auth_code"`
	AccessToken  string `json:"access_token"`
	RefreshToken string `json:"refresh_token"`
	ExpiresIn    int64  `json:"expires_in"`
}

type FolderListRequest struct {
	AccountID string `json:"account_id"`
	Refresh   bool   `json:"refresh"`
}

type FolderCreateRequest struct {
	AccountID string `json:"account_id"`
	Name      string `json:"name"`
}

type ThreadListRequest struct {
	AccountID    string `json:"account_id"`
	FolderID     string `json:"folder_id"`
	Query        string `json:"query"`
	Filter       string `json:"filter"`
	BeforeCursor string `json:"before_cursor"`
	Refresh      bool   `json:"refresh"`
}

type AttachmentInput struct {
	Filename string `json:"filename"`
	Mime     string `json:"mime"`
	Data     string `json:"data"` // base64 encoded
	InlineID string `json:"inline_id"`
}

type SendMailRequest struct {
	AccountID   string            `json:"account_id"`
	To          string            `json:"to"`
	Cc          string            `json:"cc"`
	Bcc         string            `json:"bcc"`
	Subject     string            `json:"subject"`
	Body        string            `json:"body"`
	Html        string            `json:"html"`
	InReplyTo   string            `json:"in_reply_to"`
	References  string            `json:"references"`
	ReplyTo     string            `json:"reply_to"`
	From        string            `json:"from"`
	DraftID     string            `json:"draft_id"`
	MessageID   string            `json:"message_id"`
	Attachments []AttachmentInput `json:"attachments"`
}

type ExchangedProfile struct {
	Email        string `json:"email"`
	DisplayName  string `json:"display_name"`
	AvatarURL    string `json:"avatar_url"`
	AccessToken  string `json:"access_token"`
	RefreshToken string `json:"refresh_token"`
	ExpiresIn    int64  `json:"expires_in"`
	AuthCode     string `json:"auth_code"`
}

type TokenExchangeResult struct {
	AccessToken  string `json:"access_token"`
	RefreshToken string `json:"refresh_token"`
	ExpiresIn    int64  `json:"expires_in"`
	// IDToken is the OIDC id_token Microsoft returns; Google omits it. Used to
	// read the account email/name without an extra userinfo call.
	IDToken string `json:"id_token"`
}

type GoogleUserInfo struct {
	Email   string `json:"email"`
	Name    string `json:"name"`
	Picture string `json:"picture"`
}

type ImapThreadIDs struct {
	Account   string
	Folder    string
	ThreadKey string
	UID       uint32
}

func foldersJSON(accountID string, raw any) any {
	object, _ := raw.(map[string]any)
	list, _ := object["folders"].([]any)
	folders := make([]Folder, 0, len(list))
	for _, item := range list {
		folderObject, _ := item.(map[string]any)
		name, _ := folderObject["name"].(string)
		if name == "" {
			continue
		}
		delimiter, _ := folderObject["delimiter"].(string)
		role := "folder"
		if strings.EqualFold(name, "INBOX") {
			role = "inbox"
		} else if looksLikeArchiveFolder(name) {
			role = "archive"
		}
		unread := uint32(jsonNumber(folderObject["unread"]))
		folders = append(folders, Folder{ID: name, AccountID: accountID, Name: name, Role: role, Delimiter: delimiter, Unread: unread})
	}
	return map[string]any{"folders": folders}
}

func looksLikeArchiveFolder(name string) bool {
	switch strings.ToLower(name) {
	case "archive", "archives", "all mail", "inbox.archive", "inbox.archives", "[gmail]/all mail", "[google mail]/all mail":
		return true
	default:
		return false
	}
}

func threadsJSON(accountID, folder string, raw any) any {
	object, _ := raw.(map[string]any)
	list, _ := object["messages"].([]any)
	type threadGroup struct {
		message Message
	}
	groups := make(map[string]*threadGroup)
	order := make([]string, 0, len(list))

	// Determine the original root message's subject for each threadKey. The
	// display variant preserves real tags (used for the card title); the grouping
	// variant strips gateway tags (used to match compound keys / branch back-pointers).
	oldestDisplaySubject := make(map[string]string)
	oldestGroupSubject := make(map[string]string)
	oldestUID := make(map[string]int64)
	for _, item := range list {
		msg, _ := item.(map[string]any)
		uid := jsonNumber(msg["uid"])
		if uid <= 0 {
			continue
		}
		msgFolder := jsonString(msg["folder"])
		if msgFolder == "" {
			msgFolder = folder
		}
		threadKey := jsonString(msg["thread_key"])
		if threadKey == "" {
			threadKey = fmt.Sprintf("uid:%d", uid)
		}
		currOldestUID, exists := oldestUID[threadKey]
		if !exists || uid < currOldestUID {
			oldestUID[threadKey] = uid
			oldestDisplaySubject[threadKey] = normalizeThreadSubject(jsonString(msg["subject"]))
			oldestGroupSubject[threadKey] = threadGroupingSubject(jsonString(msg["subject"]))
		}
	}

	for _, item := range list {
		msg, _ := item.(map[string]any)
		uid := jsonNumber(msg["uid"])
		if uid <= 0 {
			continue
		}
		msgFolder := jsonString(msg["folder"])
		if msgFolder == "" {
			msgFolder = folder
		}
		threadKey := jsonString(msg["thread_key"])
		if threadKey == "" {
			threadKey = fmt.Sprintf("uid:%d", uid)
		}
		compoundKey := threadKey
		normSub := ""
		if !strings.HasPrefix(threadKey, "uid:") {
			normSub = threadGroupingSubject(jsonString(msg["subject"]))
			compoundKey = threadKey + "#" + normSub
		}
		group, exists := groups[compoundKey]
		if !exists {
			threadID := formatImapThreadID(accountID, msgFolder, compoundKey)

			// Compute OriginalThreadID if this is a branched thread
			var originalThreadID string
			if !strings.HasPrefix(threadKey, "uid:") {
				origSubject := oldestGroupSubject[threadKey]
				if normSub != origSubject {
					originalThreadID = formatImapThreadID(accountID, msgFolder, threadKey+"#"+origSubject)
				}
			}

			// Thread title is the *root* message's normalized subject (without
			// "Re:"/"Fwd:" prefixes) so the list shows the original conversation
			// title regardless of which message we encountered first. Falls back
			// to this message's raw subject only when normalization stripped
			// everything (e.g. an isolated "Re:" with nothing after).
			titleSubject := oldestDisplaySubject[threadKey]
			if titleSubject == "" {
				titleSubject = jsonString(msg["subject"])
			}
			group = &threadGroup{message: Message{
				ID:                threadID,
				AccountID:         accountID,
				FolderID:          msgFolder,
				ThreadID:          threadID,
				FromName:          jsonString(msg["from_name"]),
				FromAddr:          jsonString(msg["from_addr"]),
				Subject:           titleSubject,
				Date:              jsonNumber(msg["date"]),
				OriginalThreadID:  originalThreadID,
				RecipientOverflow: uint32(jsonNumber(msg["recipient_overflow"])),
			}}
			groups[compoundKey] = group
			order = append(order, compoundKey)
		}
		if !jsonBool(msg["seen"]) {
			group.message.Unread = true
			group.message.UnreadCount++
		}
		if jsonBool(msg["starred"]) {
			group.message.Starred = true
		}
	}
	messages := make([]Message, 0, len(order))
	for _, key := range order {
		messages = append(messages, groups[key].message)
	}
	out := map[string]any{"threads": messages}
	if cursor, _ := object["next_cursor"].(string); cursor != "" {
		out["next_cursor"] = cursor
	}
	return out
}

func threadMessagesJSON(accountID, threadID, folder string, raw any, subjectFilter string) any {
	object, _ := raw.(map[string]any)
	list, _ := object["messages"].([]any)
	messages := make([]Message, 0, len(list))
	for _, item := range list {
		entry, _ := item.(map[string]any)
		msg, _ := entry["message"].(map[string]any)
		if subjectFilter != "" {
			normSub := threadGroupingSubject(jsonString(msg["subject"]))
			if normSub != subjectFilter {
				continue
			}
		}
		uid := jsonNumber(entry["uid"])
		attachments := msg["attachments"]
		attachmentList, _ := attachments.([]any)
		messageID := fmt.Sprintf("%s#%d", threadID, uid)
		// The thread spans folders; each message keeps its own source folder so
		// per-message actions (e.g. single-message delete) target the right
		// mailbox rather than the thread's nominal folder.
		msgFolder := jsonString(entry["folder"])
		if msgFolder == "" {
			msgFolder = folder
		}
		messages = append(messages, Message{
			ID:             messageID,
			AccountID:      accountID,
			FolderID:       msgFolder,
			ThreadID:       threadID,
			FromName:       jsonString(msg["from_name"]),
			FromAddr:       jsonString(msg["from_addr"]),
			To:             jsonString(msg["to"]),
			ReplyTo:        jsonString(msg["reply_to"]),
			Cc:             jsonString(msg["cc"]),
			Bcc:            jsonString(msg["bcc"]),
			MessageID:      jsonString(msg["message_id"]),
			References:     jsonString(msg["references"]),
			Subject:        jsonString(msg["subject"]),
			Preview:        jsonString(msg["preview"]),
			Body:           jsonString(msg["body"]),
			BodyHTML:       jsonString(msg["body_html"]),
			Date:           jsonNumber(msg["date"]),
			Unread:         !jsonBool(entry["seen"]),
			Starred:        jsonBool(entry["starred"]),
			HasAttachments: len(attachmentList) > 0,
			Attachments:    attachmentList,
		})
	}
	out := map[string]any{"messages": messages}
	if cursor, _ := object["next_cursor"].(string); cursor != "" {
		out["next_cursor"] = cursor
	}
	return out
}

func messageJSON(accountID, threadID, folder string, raw any) any {
	object, _ := raw.(map[string]any)
	msg, _ := object["message"].(map[string]any)
	attachments := msg["attachments"]
	attachmentList, _ := attachments.([]any)
	return map[string]any{"messages": []Message{{
		ID:             threadID,
		AccountID:      accountID,
		FolderID:       folder,
		ThreadID:       threadID,
		FromName:       jsonString(msg["from_name"]),
		FromAddr:       jsonString(msg["from_addr"]),
		To:             jsonString(msg["to"]),
		ReplyTo:        jsonString(msg["reply_to"]),
		Cc:             jsonString(msg["cc"]),
		Bcc:            jsonString(msg["bcc"]),
		MessageID:      jsonString(msg["message_id"]),
		References:     jsonString(msg["references"]),
		Subject:        jsonString(msg["subject"]),
		Preview:        jsonString(msg["preview"]),
		Body:           jsonString(msg["body"]),
		BodyHTML:       jsonString(msg["body_html"]),
		Date:           jsonNumber(msg["date"]),
		HasAttachments: len(attachmentList) > 0,
		Attachments:    attachmentList,
	}}}
}

func parseImapThreadID(threadID string) (ImapThreadIDs, bool) {
	first := strings.Index(threadID, "#")
	last := strings.LastIndex(threadID, "#")
	if first <= 0 || last <= first {
		return ImapThreadIDs{}, false
	}
	keyPart := threadID[last+1:]
	if encoded, ok := strings.CutPrefix(keyPart, "t."); ok {
		decoded, err := base64.RawURLEncoding.DecodeString(encoded)
		if err != nil {
			return ImapThreadIDs{}, false
		}
		return ImapThreadIDs{Account: threadID[:first], Folder: threadID[first+1 : last], ThreadKey: string(decoded)}, true
	}
	uid, err := strconv.ParseUint(keyPart, 10, 32)
	if err != nil {
		return ImapThreadIDs{}, false
	}
	return ImapThreadIDs{Account: threadID[:first], Folder: threadID[first+1 : last], UID: uint32(uid)}, true
}

// canonThreadFolder mirrors the sidecar's canon_folder so a thread always gets
// the same id no matter which path mints it. Without this the inbox is spelled
// "inbox" by the thread-list builder (the UI folder id) and "INBOX" by the
// notification/cached-row paths, so a notification-opened thread's id never
// matches its list card — selection/reply-target lookups then fall back to the
// header-less card and replies go out unthreaded.
func canonThreadFolder(folder string) string {
	if strings.EqualFold(folder, "inbox") {
		return "INBOX"
	}
	return folder
}

func formatImapThreadID(accountID, folder, threadKey string) string {
	encoded := base64.RawURLEncoding.EncodeToString([]byte(threadKey))
	return fmt.Sprintf("%s#%s#t.%s", accountID, canonThreadFolder(folder), encoded)
}

func formatParsedImapThreadIDInFolder(ids ImapThreadIDs, folder string) string {
	if ids.ThreadKey == "" && ids.UID > 0 {
		return fmt.Sprintf("%s#%s#%d", ids.Account, canonThreadFolder(folder), ids.UID)
	}
	return formatImapThreadID(ids.Account, folder, ids.ThreadKey)
}

func withMovedThreadLocation(res any, ids ImapThreadIDs, folderField string) map[string]any {
	out, _ := res.(map[string]any)
	if out == nil {
		out = map[string]any{"ok": true}
	}
	folder := jsonString(out[folderField])
	if folder != "" {
		out["thread_id"] = formatParsedImapThreadIDInFolder(ids, folder)
	}
	return out
}

var subjectPrefixRegex = regexp.MustCompile(`(?i)^(?:\[[^\]]+\]\s*)*(?:re|fw|fwd|aw|sv|vs|rv|res|tr|antw|wg|答复|回复|转发)(?:\[[0-9]+\]|\([0-9]+\))?[:：]\s*`)

func normalizeThreadSubject(subject string) string {
	subject = strings.TrimSpace(subject)
	for {
		loc := subjectPrefixRegex.FindStringIndex(subject)
		if loc == nil || loc[0] != 0 {
			break
		}
		subject = strings.TrimSpace(subject[loc[1]:])
	}
	return subject
}

var leadingBracketTagRegex = regexp.MustCompile(`^\s*\[[^\]]*\]\s*`)

// threadGroupingSubject normalizes a subject for thread grouping/matching only.
// In addition to Re:/Fwd: prefixes it strips ANY leading bracketed tag (e.g.
// gateway-injected "[EXTERNAL]", "[CAUTION]") so a tagged inbound copy threads
// with its untagged Sent counterpart. Not used for display — see
// normalizeThreadSubject for the title-facing variant that preserves real tags.
func threadGroupingSubject(subject string) string {
	subject = strings.TrimSpace(subject)
	for {
		if loc := subjectPrefixRegex.FindStringIndex(subject); loc != nil && loc[0] == 0 {
			subject = strings.TrimSpace(subject[loc[1]:])
			continue
		}
		if loc := leadingBracketTagRegex.FindStringIndex(subject); loc != nil && loc[0] == 0 {
			subject = strings.TrimSpace(subject[loc[1]:])
			continue
		}
		break
	}
	return subject
}

func decode(payload map[string]any, out any) error {
	data, err := json.Marshal(payload)
	if err != nil {
		return err
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.UseNumber()
	return decoder.Decode(out)
}

func fetchGoogleUserInfo(accessToken string) (*GoogleUserInfo, error) {
	req, _ := http.NewRequest(http.MethodGet, "https://www.googleapis.com/oauth2/v3/userinfo", nil)
	req.Header.Set("Authorization", "Bearer "+accessToken)
	res, err := http.DefaultClient.Do(req)
	if err != nil {
		return nil, err
	}
	defer res.Body.Close()
	body, _ := io.ReadAll(res.Body)
	if res.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("Google userinfo failed: %s", strings.TrimSpace(string(body)))
	}
	var out GoogleUserInfo
	return &out, json.Unmarshal(body, &out)
}

// accountID derives the stable primary key for a mail account from its email.
// The normalized address is used verbatim: a SQLite TEXT key accepts any
// character, and keeping the address intact avoids collisions (an earlier
// scheme that mapped @ . + _ all to '-' folded a.b@x.com and a-b@x.com onto the
// same id). Mail ids always contain '@', so they never collide with the
// "rss-" namespace minted by the sidecar (see isRSSAccountID).
func accountID(email string) string {
	return strings.ToLower(strings.TrimSpace(email))
}

// tlsMode maps a port to a transport security choice. With TLS enabled, the
// standard submission/IMAPS ports (993, 465) use implicit TLS, the cleartext
// submission ports (143, 25, 587) upgrade via STARTTLS, and the local test
// ports stay plaintext. Returns (implicitTLS, starttls); at most one is true.
func tlsMode(tls bool, port uint16) (bool, bool) {
	if !tls {
		return false, false
	}
	switch port {
	case 3143, 3587:
		return false, false
	case 143, 25, 587:
		return false, true
	default:
		return true, false
	}
}

func randomBase64URL(length int) string {
	buf := make([]byte, length)
	_, _ = rand.Read(buf)
	return base64.RawURLEncoding.EncodeToString(buf)
}

func loadDotEnvLocal() {
	data, err := os.ReadFile(".env.local")
	if err != nil {
		return
	}
	for _, rawLine := range strings.Split(string(data), "\n") {
		line := strings.TrimSpace(rawLine)
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		line = strings.TrimPrefix(line, "export ")
		key, value, ok := strings.Cut(line, "=")
		if !ok || os.Getenv(strings.TrimSpace(key)) != "" {
			continue
		}
		value = strings.Trim(strings.TrimSpace(value), `"'`)
		_ = os.Setenv(strings.TrimSpace(key), value)
	}
}

// coreBinaryPath returns the path to the Rust core engine sidecar, resolving it
// once and caching the result. Resolution order:
//
//  1. MERON_CORE_SERVER env var (explicit override; used in dev and custom installs)
//  2. the embedded sidecar, extracted to the cache dir (release builds)
//  3. the freshly built debug binary relative to the repo root (plain `wails dev`)
//
// Resolving relative to the working directory was the cause of the release
// binary showing the setup screen: launched outside the repo, it never found
// the sidecar, so account.list returned empty.
func coreBinaryPath() string {
	mailServerOnce.Do(func() {
		mailServerResolved = resolveMailServerPath()
	})
	return mailServerResolved
}

var (
	mailServerOnce     sync.Once
	mailServerResolved string
)

func resolveMailServerPath() string {
	if value := os.Getenv("MERON_CORE_SERVER"); value != "" {
		return value
	}
	if len(embeddedSidecar) > 0 {
		if path, err := extractEmbeddedSidecar(); err == nil {
			return path
		} else {
			fmt.Fprintf(os.Stderr, "meron: failed to extract embedded sidecar: %v\n", err)
		}
	}
	return "meron-core/target/debug/meron-core"
}

// extractEmbeddedSidecar writes the embedded sidecar to the cache dir under a
// content-hashed filename, so repeated launches reuse the same file and a new
// build lands in a new file without clobbering a running one. The write is
// atomic (temp file + rename) to survive concurrent or interrupted launches.
func extractEmbeddedSidecar() (string, error) {
	sum := sha256.Sum256(embeddedSidecar)
	name := "meron-core-" + hex.EncodeToString(sum[:8])
	dir := filepath.Join(appCacheDir(), "bin")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return "", err
	}
	path := filepath.Join(dir, name)
	if info, err := os.Stat(path); err == nil && info.Size() == int64(len(embeddedSidecar)) {
		return path, nil
	}
	tmp, err := os.CreateTemp(dir, name+".tmp-*")
	if err != nil {
		return "", err
	}
	tmpName := tmp.Name()
	if _, err := tmp.Write(embeddedSidecar); err != nil {
		tmp.Close()
		os.Remove(tmpName)
		return "", err
	}
	if err := tmp.Chmod(0o755); err != nil {
		tmp.Close()
		os.Remove(tmpName)
		return "", err
	}
	if err := tmp.Close(); err != nil {
		os.Remove(tmpName)
		return "", err
	}
	if err := os.Rename(tmpName, path); err != nil {
		os.Remove(tmpName)
		return "", err
	}
	return path, nil
}

// mediaHandler serves attachment files written by the sidecar at `/media/<key>`,
// so image bytes load straight into the webview instead of round-tripping as
// base64 through the JSON bridge. The AssetServer tries embedded frontend assets
// first and only falls through here for unmatched paths. http.Dir cleans the
// path and blocks `..` traversal out of the media root.
func mediaHandler() http.Handler {
	fs := http.StripPrefix("/media/", http.FileServer(http.Dir(mediaDir())))
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if !strings.HasPrefix(r.URL.Path, "/media/") {
			http.NotFound(w, r)
			return
		}
		fs.ServeHTTP(w, r)
	})
}

// mediaDir is where the mail sidecar writes attachment files and the asset
// handler serves them from at `/media/<key>`. Both sides must agree, so the
// bridge sets MERON_MEDIA_DIR on the sidecar to this exact path.
func mediaDir() string {
	return filepath.Join(appCacheDir(), "attachments")
}

// storageUsage reports disk used by the clearable attachment cache and by the
// SQLite DB. They are returned separately so the UI can offer to clear the
// cache while making clear the DB (the user's mail) is not touched here.
func (a *App) storageUsage() (any, error) {
	return map[string]any{
		"cacheBytes": dirSize(mediaDir()),
		"dbBytes":    fileSize(filepath.Join(appConfigDir(), "meron.db")),
	}, nil
}

// storageClearCache deletes the attachment cache and recreates the empty dir so
// the sidecar can keep writing. It never touches the DB.
func (a *App) storageClearCache() (any, error) {
	dir := mediaDir()
	if err := os.RemoveAll(dir); err != nil {
		return nil, err
	}
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return nil, err
	}
	return a.storageUsage()
}

// dirSize sums the sizes of all regular files under root. A missing dir is 0.
func dirSize(root string) int64 {
	var total int64
	_ = filepath.WalkDir(root, func(_ string, d os.DirEntry, err error) error {
		if err != nil || d.IsDir() {
			return nil
		}
		if info, err := d.Info(); err == nil {
			total += info.Size()
		}
		return nil
	})
	return total
}

// fileSize returns the size of a single file, or 0 if it is missing.
func fileSize(path string) int64 {
	if info, err := os.Stat(path); err == nil {
		return info.Size()
	}
	return 0
}

func appConfigDir() string {
	return filepath.Join(configHome(), appDirName())
}

func appCacheDir() string {
	return filepath.Join(cacheHome(), appDirName())
}

func appDirName() string {
	if isWailsDevLaunch() {
		return "meron-dev"
	}
	return "meron"
}

func appUniqueID() string {
	if isWailsDevLaunch() {
		return "jp.nonbili.meron-dev"
	}
	return "jp.nonbili.meron"
}

func isWailsDevLaunch() bool {
	return os.Getenv("devserver") != "" || os.Getenv("frontenddevserverurl") != ""
}

func cacheHome() string {
	if dir, err := os.UserCacheDir(); err == nil {
		return dir
	}
	return "."
}

func configHome() string {
	if dir, err := os.UserConfigDir(); err == nil {
		return dir
	}
	return "."
}

// maxLogBytes caps the on-disk log. Rotation happens once, at open: if the file
// is over the cap it's renamed to meron.log.1 (replacing any prior .1) and a fresh
// file is started. A desktop app restarts often enough that open-time rotation is
// sufficient to keep the log from growing without bound.
const maxLogBytes = 5 << 20

func openAppLog() (*log.Logger, *os.File) {
	path := filepath.Join(appConfigDir(), "meron.log")
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return log.New(os.Stderr, "meron: ", log.LstdFlags|log.Lmicroseconds), nil
	}
	if info, err := os.Stat(path); err == nil && info.Size() > maxLogBytes {
		_ = os.Rename(path, path+".1")
	}
	file, err := os.OpenFile(path, os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0o600)
	if err != nil {
		return log.New(os.Stderr, "meron: ", log.LstdFlags|log.Lmicroseconds), nil
	}
	return log.New(io.MultiWriter(os.Stderr, file), "meron: ", log.LstdFlags|log.Lmicroseconds), file
}

func (a *App) logWriter() io.Writer {
	if a.logFile == nil {
		return os.Stderr
	}
	return io.MultiWriter(os.Stderr, a.logFile)
}

func (a *App) logf(format string, args ...any) {
	if a.logger != nil {
		a.logger.Printf(format, args...)
	}
}

func sidecarErrorMessage(value any) string {
	switch typed := value.(type) {
	case map[string]any:
		if message, ok := typed["message"].(string); ok {
			return message
		}
		if data, err := json.Marshal(typed); err == nil {
			return string(data)
		}
	case string:
		return typed
	}
	return fmt.Sprint(value)
}

func sidecarCallTimeout(method string) time.Duration {
	switch method {
	case "account.connect":
		return 45 * time.Second
	case "messages.read", "messages.thread", "messages.markRead", "messages.markAllRead", "send", "save_draft":
		return 30 * time.Second
	case "folders.list", "folders.create", "folders.archive", "messages.recent", "messages.sync", "rss.markRead", "watch.start", "watch.stop", "discard_draft", "account.addRss", "feed.add", "rss.importOpml":
		return 15 * time.Second
	default:
		return 5 * time.Second
	}
}

func fileExists(path string) bool {
	if path == "" {
		return false
	}
	_, err := os.Stat(path)
	return err == nil
}

func jsonString(value any) string {
	out, _ := value.(string)
	return out
}

func jsonBool(value any) bool {
	out, _ := value.(bool)
	return out
}

func jsonNumber(value any) int64 {
	switch v := value.(type) {
	case float64:
		return int64(v)
	case json.Number:
		n, _ := v.Int64()
		return n
	case int64:
		return v
	default:
		return 0
	}
}
