package main

import (
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

	err := wails.Run(&options.App{
		Title:                    "Meron",
		Width:                    1200,
		Height:                   800,
		WindowStartState:         options.Maximised,
		HideWindowOnClose:        true,
		EnableDefaultContextMenu: true,
		AssetServer: &assetserver.Options{
			Assets:     assets,
			Handler:    mediaHandler(),
			Middleware: cspMiddleware,
		},
		Linux: &linux.Options{
			Icon: appIconPNG,
		},
		Mac: &mac.Options{
			// Hide the native title bar and extend content to the top edge,
			// keeping the traffic-light buttons. The frontend draws a slim
			// draggable strip in their place (see MacTitleBar).
			TitleBar:  mac.TitleBarHidden(),
			OnUrlOpen: app.openMailtoURL,
		},
		SingleInstanceLock: &options.SingleInstanceLock{
			UniqueId: appUniqueID(),
			OnSecondInstanceLaunch: func(data options.SecondInstanceData) {
				app.HandleSecondInstanceLaunch(data.Args)
			},
		},
		OnStartup:  app.Startup,
		OnShutdown: app.Shutdown,
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
