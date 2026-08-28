//go:build linux && !bindings

package main

/*
#cgo pkg-config: gtk+-3.0

#include <gtk/gtk.h>

static gboolean applyPreferDarkTheme(gpointer data) {
	GtkSettings *settings = gtk_settings_get_default();
	if (settings != NULL) {
		g_object_set(settings, "gtk-application-prefer-dark-theme", data != NULL ? TRUE : FALSE, NULL);
	}
	return G_SOURCE_REMOVE;
}

static void setPreferDarkTheme(int dark) {
	g_idle_add(applyPreferDarkTheme, dark ? GINT_TO_POINTER(1) : NULL);
}
*/
import "C"

// setNativeWindowDark switches GTK to the dark variant of the current theme.
//
// That is what decorates the title bar: with client-side decorations GTK draws
// the header bar itself, and under a window manager GTK reflects the preference
// onto the toplevel's _GTK_THEME_VARIANT so the WM picks its dark frame.
//
// The property is set from a GTK idle callback because invoke() runs on a
// request goroutine, not the thread owning the GTK main loop.
func setNativeWindowDark(dark bool) {
	value := C.int(0)
	if dark {
		value = 1
	}
	C.setPreferDarkTheme(value)
}
