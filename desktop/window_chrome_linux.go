//go:build linux && !bindings

package main

/*
#cgo pkg-config: gtk+-3.0

#include <stdlib.h>
#include <gtk/gtk.h>
#ifdef GDK_WINDOWING_WAYLAND
#include <gdk/gdkwayland.h>
#endif

static GtkWindow *mainWindow = NULL;
static gboolean wantIntegrated = FALSE;
static gboolean waitStartupMaximise = FALSE;
static gboolean startupReady = FALSE;
static gboolean startupRevealed = FALSE;

static void revealStartupIfReady(void) {
	if (mainWindow == NULL || startupRevealed || !startupReady || waitStartupMaximise) return;
	gtk_widget_set_opacity(GTK_WIDGET(mainWindow), 1.0);
	startupRevealed = TRUE;
}

static GMutex chromeMu;
static gchar *decorationLayout = NULL;
static gchar *doubleClickAction = NULL;

// GtkSettings mirrors GNOME's org.gnome.desktop.wm.preferences button-layout
// and action-double-click-titlebar; keep a copy the Go side can read from any
// thread.
static void readChromeSettings(void) {
	GtkSettings *settings = gtk_settings_get_default();
	if (settings == NULL) return;
	gchar *layout = NULL, *doubleClick = NULL;
	g_object_get(settings, "gtk-decoration-layout", &layout, "gtk-titlebar-double-click", &doubleClick, NULL);
	g_mutex_lock(&chromeMu);
	g_free(decorationLayout);
	decorationLayout = layout;
	g_free(doubleClickAction);
	doubleClickAction = doubleClick;
	g_mutex_unlock(&chromeMu);
}

static void onChromeSettingChanged(GObject *object, GParamSpec *pspec, gpointer data) {
	readChromeSettings();
}

// An empty custom titlebar that is never shown: GTK keeps its client-side
// decorations (shadow, resize edges outside the window, tiling) but draws no
// title bar, so the page's own title bar (TitleBar.tsx) takes its place.
//
// Switching back sets a title bar built the way GTK builds its default one
// (gtkwindow.c, create_titlebar): unsetting the custom one instead left the
// webview black until restart.
//
// Wails caps the window at the monitor size plus a frame allowance it measures
// once from GTK's default frame (SetMinMaxSize in its window.c). The
// integrated frame's libadwaita shadow is wider, so that cap left a maximised
// window short of the right screen edge (1500x924 on a 1536x960 monitor).
// Meron sets no size limits of its own, so the hints are dropped. Clearing
// them once at startup was not enough: they were back by the time the window
// was maximised (Wails re-applies them after leaving fullscreen; what else
// does was not pinned down), so they are cleared on every state change too.
static void clearSizeHints(void) {
	if (mainWindow != NULL) gtk_window_set_geometry_hints(mainWindow, NULL, NULL, 0);
}

// Whether the window is tiled (snapped to a screen edge), which Wails doesn't
// report: its IsNormal() is true for a half-screen window. Written on the GTK
// thread, read from Go.
static volatile gint tiled = 0;

static const GdkWindowState TILED_STATES = GDK_WINDOW_STATE_TILED | GDK_WINDOW_STATE_TOP_TILED |
	GDK_WINDOW_STATE_RIGHT_TILED | GDK_WINDOW_STATE_BOTTOM_TILED | GDK_WINDOW_STATE_LEFT_TILED;

static int isTiled(void) {
	return g_atomic_int_get(&tiled);
}

// Defined in Go (window_chrome_linux_export.go).
extern void goWindowStateChanged(void);
extern void goWindowTitlebarUnavailable(void);

// Tiling can land after the page's resize event, so the page is told to ask
// again (window.stateChanged) once GTK has the new state.
static gboolean onWindowState(GtkWidget *widget, GdkEventWindowState *event, gpointer data) {
	g_atomic_int_set(&tiled, (event->new_window_state & TILED_STATES) != 0);
	if (wantIntegrated) clearSizeHints();
	if (event->new_window_state & GDK_WINDOW_STATE_MAXIMIZED) {
		waitStartupMaximise = FALSE;
		revealStartupIfReady();
	}
	goWindowStateChanged();
	return FALSE;
}

static gboolean maximiseAfterConfigure = FALSE;

static gboolean onBackgroundConfigure(GtkWidget *widget, GdkEventConfigure *event, gpointer data) {
	if (maximiseAfterConfigure && gtk_widget_get_mapped(widget)) {
		// The window manager has now configured the normal window and can
		// retain its restore geometry before accepting maximisation.
		maximiseAfterConfigure = FALSE;
		gtk_window_maximize(GTK_WINDOW(widget));
	}
	return FALSE;
}

static void maximiseOnNextConfigure(void) {
	maximiseAfterConfigure = TRUE;
}

// Whether the current GTK frame has transparent client-side decorations.
// Re-read after installing a custom titlebar: KDE defaults to server-side
// decorations, but a custom titlebar asks GTK to take ownership of the frame.
// Without compositing GTK may use solid-csd, which must keep square corners.
static volatile gint gtkFrame = 0;
static volatile gint liveTitlebar = 0;

static void updateFrame(void) {
	GtkStyleContext *style = gtk_widget_get_style_context(GTK_WIDGET(mainWindow));
	g_atomic_int_set(&gtkFrame, gtk_style_context_has_class(style, "csd") && !gtk_style_context_has_class(style, "solid-csd"));
}

static int drawsFrame(void) {
	return g_atomic_int_get(&gtkFrame);
}

static int switchesTitlebarLive(void) {
	// Without transparent CSD, GTK may not have an internal titlebar to
	// swap. Keep that transition at startup too.
	return g_atomic_int_get(&liveTitlebar) && drawsFrame();
}

static void applyTitlebar(void) {
	if (mainWindow == NULL) return;
	clearSizeHints();
	if (wantIntegrated) {
		GtkWidget *current = gtk_window_get_titlebar(mainWindow);
		GtkStyleContext *style = gtk_widget_get_style_context(GTK_WIDGET(mainWindow));
		// If preparation missed this window, adding its first titlebar now
		// would unrealize it inside the realize signal. Keep the native bar.
		// A preinstalled solid-csd titlebar is safe even without compositing.
		if (gtk_widget_get_realized(GTK_WIDGET(mainWindow)) && current == NULL &&
			!gtk_style_context_has_class(style, "csd")) {
			wantIntegrated = FALSE;
			goWindowTitlebarUnavailable();
			updateFrame();
			return;
		}
		if (current != NULL && !gtk_widget_get_visible(current)) {
			updateFrame();
			return;
		}
		GtkWidget *bar = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 0);
		gtk_widget_set_no_show_all(bar, TRUE);
		gtk_window_set_titlebar(mainWindow, bar);
	} else if (gtk_window_get_titlebar(mainWindow) != NULL) {
		// As create_titlebar sets it up. has-subtitle defaults to TRUE, which
		// reserves a subtitle line: 47px tall instead of GTK's 37px.
		GtkWidget *bar = gtk_header_bar_new();
		g_object_set(bar, "spacing", 0, "has-subtitle", FALSE, NULL);
		gtk_header_bar_set_title(GTK_HEADER_BAR(bar), gtk_window_get_title(mainWindow));
		gtk_header_bar_set_show_close_button(GTK_HEADER_BAR(bar), TRUE);
		gtk_style_context_add_class(gtk_widget_get_style_context(bar), "default-decoration");
		gtk_widget_show(bar);
		gtk_window_set_titlebar(mainWindow, bar);
	}
	updateFrame();
}

// Wails has no callback between creating and showing its GtkWindow. GTK
// validates the window's style before realizing it, so style-updated lets us
// install the custom titlebar before the GdkWindow and WebKit child exist.
// Installing the first titlebar from realize is too late: GTK unrealizes the
// window while other realize handlers still expect a valid GdkWindow.
static void prepareTitlebar(GtkWidget *widget) {
	// Setting the titlebar may validate styles again before this hook returns.
	static gboolean prepared = FALSE;
	if (prepared) return;
	prepared = TRUE;
	GdkDisplay *display = gtk_widget_get_display(widget);
	gboolean defaultCsd = g_strcmp0(g_getenv("GTK_CSD"), "1") == 0;
#ifdef GDK_WINDOWING_WAYLAND
	if (GDK_IS_WAYLAND_DISPLAY(display)) defaultCsd = !gdk_wayland_display_prefers_ssd(display);
#endif
	// KDE and default X11 windows need a restart to return to the desktop's
	// native frame. GNOME's two GTK titlebars can be swapped live.
	g_atomic_int_set(&liveTitlebar, defaultCsd);
	if (wantIntegrated) {
		GtkWidget *bar = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 0);
		gtk_widget_set_no_show_all(bar, TRUE);
		gtk_window_set_titlebar(GTK_WINDOW(widget), bar);
	}
}

static gboolean onStyleUpdated(GSignalInvocationHint *hint, guint n, const GValue *params, gpointer data) {
	GObject *object = g_value_get_object(&params[0]);
	if (!GTK_IS_WINDOW(object) || gtk_window_get_window_type(GTK_WINDOW(object)) != GTK_WINDOW_TOPLEVEL) return TRUE;
	prepareTitlebar(GTK_WIDGET(object));
	return FALSE;
}

// The realized window owns the native state callbacks and startup reveal.
static gboolean onRealize(GSignalInvocationHint *hint, guint n, const GValue *params, gpointer data) {
	GObject *object = g_value_get_object(&params[0]);
	if (!GTK_IS_WINDOW(object) || gtk_window_get_window_type(GTK_WINDOW(object)) != GTK_WINDOW_TOPLEVEL) {
		return TRUE;
	}
	mainWindow = GTK_WINDOW(object);
	// Conceal the initial normal-size window without unmapping it: maximising
	// an unmapped window loses the restore geometry on GNOME. React reveals
	// it after its first frame, while WebKit continues rendering underneath.
	gtk_widget_set_opacity(GTK_WIDGET(object), 0.0);
	g_object_add_weak_pointer(object, (gpointer *)&mainWindow);
	g_signal_connect(object, "window-state-event", G_CALLBACK(onWindowState), NULL);
	g_signal_connect(object, "configure-event", G_CALLBACK(onBackgroundConfigure), NULL);
	readChromeSettings();
	GtkSettings *settings = gtk_settings_get_default();
	if (settings != NULL) {
		g_signal_connect(settings, "notify::gtk-decoration-layout", G_CALLBACK(onChromeSettingChanged), NULL);
		g_signal_connect(settings, "notify::gtk-titlebar-double-click", G_CALLBACK(onChromeSettingChanged), NULL);
	}
	applyTitlebar();
	revealStartupIfReady();
	return FALSE;
}

static gulong installWindowChrome(int integrated) {
	wantIntegrated = integrated ? TRUE : FALSE;
	// Before gtk_init the widget classes aren't loaded and the lookup fails.
	g_type_class_unref(g_type_class_ref(GTK_TYPE_WINDOW));
	guint styleId = g_signal_lookup("style-updated", GTK_TYPE_WIDGET);
	gulong styleHook = g_signal_add_emission_hook(styleId, 0, onStyleUpdated, NULL, NULL);
	guint id = g_signal_lookup("realize", GTK_TYPE_WIDGET);
	g_signal_add_emission_hook(id, 0, onRealize, NULL, NULL);
	return styleHook;
}

static void prepareBackgroundWindow(GtkWidget *window) {
	// Hidden widgets have not passed through GTK's style validation yet.
	// Prepare the titlebar before realize, just as style-updated normally does.
	prepareTitlebar(window);
	gtk_widget_realize(window);
}

static gboolean setIntegratedIdle(gpointer data) {
	wantIntegrated = data != NULL;
	applyTitlebar();
	return G_SOURCE_REMOVE;
}

static void setIntegrated(int integrated) {
	g_idle_add(setIntegratedIdle, integrated ? GINT_TO_POINTER(1) : NULL);
}

static gboolean revealStartupIdle(gpointer data) {
	startupReady = TRUE;
	if (data != NULL) waitStartupMaximise = FALSE;
	revealStartupIfReady();
	return G_SOURCE_REMOVE;
}

static void revealStartupWindow(int force) {
	g_idle_add(revealStartupIdle, force ? GINT_TO_POINTER(1) : NULL);
}

// Called before wails.Run, before any GTK callbacks can read this flag.
static void expectStartupMaximise(int maximised) {
	waitStartupMaximise = maximised ? TRUE : FALSE;
}

// The window's own close, so it runs through delete-event and Wails'
// OnBeforeClose exactly like the title bar's close button.
static gboolean closeIdle(gpointer data) {
	if (mainWindow != NULL) gtk_window_close(mainWindow);
	return G_SOURCE_REMOVE;
}

static void closeMainWindow(void) {
	g_idle_add(closeIdle, NULL);
}

// Caller frees.
static char *copyChromeSetting(int doubleClick) {
	g_mutex_lock(&chromeMu);
	char *value = g_strdup(doubleClick ? doubleClickAction : decorationLayout);
	g_mutex_unlock(&chromeMu);
	return value;
}
*/
import "C"

import (
	"unsafe"

	"meron/internal/desktopruntime"

	"github.com/wailsapp/wails/v3/pkg/application"
)

const integratedTitlebarSupported = true

// GTK keeps its frame without a title bar (an empty custom one), so the window
// isn't frameless.
const framelessTitlebar = false

// Only windows that originally had a GTK titlebar can switch live without
// recreating the WebKit window. A var lets the command tests stub this query.
var titlebarSwitchesLive = func() bool { return C.switchesTitlebarLive() != 0 }

func installWindowChrome(integrated bool) {
	C.installWindowChrome(cBool(integrated))
}

func prepareBackgroundWindow() {
	// Claim the main window's one-shot realize hook before a composer or
	// document window opens, without mapping or focusing the main window.
	application.InvokeSync(func() {
		if window := desktopruntime.Window.NativeWindow(); window != nil {
			C.prepareBackgroundWindow((*C.GtkWidget)(window))
		}
	})
}

func nativeMaximiseOnNextConfigure() {
	application.InvokeSync(func() { C.maximiseOnNextConfigure() })
}

func setNativeTitlebarIntegrated(integrated bool) {
	C.setIntegrated(cBool(integrated))
}

func nativeRevealStartupWindow(force bool) {
	C.revealStartupWindow(cBool(force))
}

func nativeExpectStartupMaximise(maximised bool) {
	C.expectStartupMaximise(cBool(maximised))
}

// nativeDrawsFrame reports whether GTK draws the window frame (see gtkFrame).
func nativeDrawsFrame() bool {
	return C.drawsFrame() != 0
}

// nativeWindowTiled reports whether the window is snapped to a screen edge.
func nativeWindowTiled() bool {
	return C.isTiled() != 0
}

func closeNativeWindow() {
	C.closeMainWindow()
}

// nativeChromeSettings returns GTK's decoration layout (e.g. "menu:close") and
// double-click action (e.g. "toggle-maximize"); empty until the window exists.
func nativeChromeSettings() (layout, doubleClick string) {
	return copyChromeSetting(0), copyChromeSetting(1)
}

func copyChromeSetting(doubleClick C.int) string {
	value := C.copyChromeSetting(doubleClick)
	if value == nil {
		return ""
	}
	defer C.g_free(C.gpointer(unsafe.Pointer(value)))
	return C.GoString(value)
}

func cBool(value bool) C.int {
	if value {
		return 1
	}
	return 0
}
