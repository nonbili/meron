//go:build linux && !bindings

package main

/*
#cgo pkg-config: gtk+-3.0

#include <stdlib.h>
#include <gtk/gtk.h>

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

// Whether GTK draws the window frame itself: client-side decorations with an
// alpha channel (the csd style class, not solid-csd). Where the desktop draws
// the frame instead (KWin on KDE Plasma, most X11 window managers) or there is
// no compositor, the window's corners are the desktop's, so the page keeps
// them square and the integrated title bar isn't offered. Recorded as the
// window is realized, before any titlebar of ours; read from Go.
static volatile gint gtkFrame = 0;

static int drawsFrame(void) {
	return g_atomic_int_get(&gtkFrame);
}

static void applyTitlebar(void) {
	if (mainWindow == NULL) return;
	if (!drawsFrame()) wantIntegrated = FALSE;
	clearSizeHints();
	if (wantIntegrated) {
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
}

// Wails creates and shows its window inside wails.Run, with no hook in
// between, so the window is picked up as it is realized. Emission hooks run
// after the class handler, so it is already realized here; setting the
// titlebar still takes effect.
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
	GtkStyleContext *style = gtk_widget_get_style_context(GTK_WIDGET(object));
	g_atomic_int_set(&gtkFrame, gtk_style_context_has_class(style, "csd") && !gtk_style_context_has_class(style, "solid-csd"));
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

static void installWindowChrome(int integrated) {
	wantIntegrated = integrated ? TRUE : FALSE;
	// Before gtk_init the widget classes aren't loaded and the lookup fails.
	g_type_class_unref(g_type_class_ref(GTK_TYPE_WINDOW));
	guint id = g_signal_lookup("realize", GTK_TYPE_WIDGET);
	g_signal_add_emission_hook(id, 0, onRealize, NULL, NULL);
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

import "unsafe"

const integratedTitlebarSupported = true

// GTK keeps its frame without a title bar (an empty custom one), so the window
// isn't frameless.
const framelessTitlebar = false

// The GTK title bar can be swapped on the live window; a var for tests.
var titlebarSwitchesLive = true

func installWindowChrome(integrated bool) {
	C.installWindowChrome(cBool(integrated))
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
