package main

import (
	"context"
	"embed"
	"io/fs"
	"log"
	"net/http"
	"runtime"
	"strings"

	"meron/internal/desktopruntime"

	"github.com/wailsapp/wails/v3/pkg/application"
	"github.com/wailsapp/wails/v3/pkg/events"
)

//go:embed all:frontend/dist
var assets embed.FS

//go:embed build/appicon.png
var appIconPNG []byte

//go:embed build/tray-color.png
var trayIconPNG []byte

//go:embed build/tray-color-unread.png
var trayIconUnreadPNG []byte

//go:embed build/trayicon-mono.png
var trayIconMonoPNG []byte

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

	native := application.New(application.Options{
		Name:                        appTitle(),
		Icon:                        appIconPNG,
		Windows:                     application.WindowsOptions{WndClass: "wailsWindow"},
		Mac:                         application.MacOptions{ApplicationShouldTerminateAfterLastWindowClosed: false},
		Services:                    []application.Service{application.NewService(&desktopService{app: app})},
		DisableDefaultSignalHandler: true,
		ShouldQuit:                  app.shouldQuit,
		Assets: application.AssetOptions{
			Handler:    desktopAssetHandler(),
			Middleware: application.ChainMiddleware(runtimeGuard, cspMiddleware),
		},
		SingleInstance: &application.SingleInstanceOptions{
			UniqueID: appUniqueID(),
			OnSecondInstanceLaunch: func(data application.SecondInstanceData) {
				go app.HandleSecondInstanceLaunch(emlArgumentsAt(data.Args, data.WorkingDir))
			},
		},
		OnShutdown: func() { app.Shutdown(app.ctx) },
	})
	desktopruntime.App = native
	native.Event.OnApplicationEvent(events.Common.ApplicationLaunchedWithUrl, func(event *application.ApplicationEvent) {
		app.openMailtoURL(event.Context().URL())
	})
	native.Event.OnApplicationEvent(events.Common.ApplicationOpenedWithFile, func(event *application.ApplicationEvent) {
		path := event.Context().Filename()
		go app.openEmlArguments([]string{path})
	})
	options := application.WebviewWindowOptions{
		Title: appTitle(), Width: app.window.Width, Height: app.window.Height,
		StartState: startWindowState(startMaximised), Hidden: startWindowHidden,
		Frameless: framelessTitlebar && integrated, EnableFileDrop: nativeFileDrop || runtime.GOOS == "windows",
		Mac: application.MacWindow{TitleBar: application.MacTitleBarHidden},
	}
	if roundedWindowCorners {
		options.BackgroundType = application.BackgroundTypeTransparent
	}
	desktopruntime.Window = native.Window.NewWithOptions(options)
	desktopruntime.Window.OnWindowEvent(events.Common.WindowRuntimeReady, func(_ *application.WindowEvent) {
		alignNativeTitlebar()
		maximiseOnDomReady(app.ctx, startMaximised)
		app.emlStartOnce.Do(app.startEmlReaders)
	})
	desktopruntime.Window.RegisterHook(events.Common.WindowClosing, func(event *application.WindowEvent) {
		// Keep the main window alive until the whole application exits. A
		// document window must never leave a destroyed main window behind.
		event.Cancel()
		app.mainWindowCloseRequested()
	})
	if err := native.Run(); err != nil {
		log.Fatal(err)
	}
}

// desktopService exposes only the command bridge, not lifecycle methods.
type desktopService struct{ app *App }

func (s *desktopService) ServiceStartup(ctx context.Context, _ application.ServiceOptions) error {
	s.app.Startup(ctx)
	return nil
}

func (s *desktopService) Invoke(command string, payload map[string]any) (any, error) {
	return s.app.Invoke(command, payload)
}

func desktopAssetHandler() http.Handler {
	return desktopAssetHandlerWithAssets(assets)
}

func desktopAssetHandlerWithAssets(assets fs.FS) http.Handler {
	frontend := application.AssetFileServerFS(assets)
	media := mediaHandler()
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if strings.HasPrefix(r.URL.Path, "/media/") {
			media.ServeHTTP(w, r)
			return
		}
		frontend.ServeHTTP(w, r)
	})
}

// contentSecurityPolicy is defined per build tag: the strict same-origin policy
// in csp_prod.go (production builds) and a relaxed policy in csp_dev.go that lets
// the Vite dev server's inline preamble and HMR websockets through. In `wails3 dev`
// the page is served from wails.localhost and proxied to Vite, so this middleware
// runs on dev responses too — hence the dev variant.
func cspMiddleware(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Security-Policy", contentSecurityPolicy)
		next.ServeHTTP(w, r)
	})
}
