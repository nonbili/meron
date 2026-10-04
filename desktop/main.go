package main

import (
	"context"
	"embed"
	"log"
	"net/http"

	"github.com/wailsapp/wails/v2"
	"github.com/wailsapp/wails/v2/pkg/options"
	"github.com/wailsapp/wails/v2/pkg/options/assetserver"
	"github.com/wailsapp/wails/v2/pkg/options/linux"
	"github.com/wailsapp/wails/v2/pkg/options/mac"
)

//go:embed all:frontend/dist
var assets embed.FS

//go:embed build/appicon.png
var appIconPNG []byte

//go:embed build/trayicon.png
var trayIconPNG []byte

//go:embed build/trayicon-unread.png
var trayIconUnreadPNG []byte

//go:embed build/trayicon.ico
var trayIconICO []byte

//go:embed build/trayicon-unread.ico
var trayIconUnreadICO []byte

var globalApp *App

func main() {
	setupNativeSpellChecking()

	app := NewApp()
	globalApp = app
	startMaximised := app.window.Maximised
	integrated := integratedTitlebarSupported && app.window.Titlebar != titlebarSystem
	integratedTitlebar.Store(integrated)
	installWindowChrome(integrated)

	err := wails.Run(&options.App{
		Title:                    appTitle(),
		Width:                    app.window.Width,
		Height:                   app.window.Height,
		WindowStartState:         startWindowState(startMaximised),
		StartHidden:              startWindowHidden,
		HideWindowOnClose:        hideOnCloseNatively,
		Frameless:                framelessTitlebar && integrated,
		EnableDefaultContextMenu: true,
		BackgroundColour:         windowBackgroundColour(),
		AssetServer: &assetserver.Options{
			Assets:     assets,
			Handler:    mediaHandler(),
			Middleware: cspMiddleware,
		},
		Linux: &linux.Options{
			Icon:                appIconPNG,
			WindowIsTranslucent: roundedWindowCorners,
		},
		Mac: &mac.Options{
			// Hide the native title bar and extend content to the top edge,
			// keeping the traffic-light buttons. TitleBar.tsx draws the 40px
			// draggable bar; alignNativeTitlebar centres the native buttons.
			TitleBar:  mac.TitleBarHidden(),
			OnUrlOpen: app.openMailtoURL,
		},
		SingleInstanceLock: &options.SingleInstanceLock{
			UniqueId: appUniqueID(),
			OnSecondInstanceLaunch: func(data options.SecondInstanceData) {
				app.HandleSecondInstanceLaunch(data.Args)
			},
		},
		OnStartup: app.Startup,
		OnDomReady: func(ctx context.Context) {
			alignNativeTitlebar()
			maximiseOnDomReady(ctx, startMaximised)
		},
		OnBeforeClose: app.beforeClose,
		OnShutdown:    app.Shutdown,
		Bind: []interface{}{
			app,
		},
	})
	if err != nil {
		log.Fatal(err)
	}
}

// contentSecurityPolicy is defined per build tag: the strict same-origin policy
// in csp_prod.go (production builds) and a relaxed policy in csp_dev.go that lets
// the Vite dev server's inline preamble and HMR websockets through. In `wails dev`
// the page is served from wails.localhost and proxied to Vite, so this middleware
// runs on dev responses too — hence the dev variant.
func cspMiddleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Security-Policy", contentSecurityPolicy)
		next.ServeHTTP(w, r)
	})
}
