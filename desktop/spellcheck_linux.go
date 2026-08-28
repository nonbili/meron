//go:build linux && webkit2_41 && !bindings

package main

/*
#cgo pkg-config: webkit2gtk-4.1

#include <glib.h>
#include <webkit2/webkit2.h>

static gboolean applyWebKitSpellChecking(gpointer data) {
	WebKitWebContext *context = webkit_web_context_get_default();
	webkit_web_context_set_spell_checking_languages(context, g_get_language_names());
	webkit_web_context_set_spell_checking_enabled(context, TRUE);
	return G_SOURCE_REMOVE;
}

static void enableWebKitSpellChecking() {
	g_idle_add(applyWebKitSpellChecking, NULL);
}
*/
import "C"

// setupNativeSpellChecking turns on WebKit's spell checker for the webview.
//
// The webkit calls are deferred onto the GTK main loop rather than made here.
// This runs from main(), before wails has called gtk_init_check and before it
// has pinned the main goroutine with LockOSThread, and
// webkit_web_context_get_default() is what *creates* the default web context —
// the same one wails later hands the webview. Creating it with no GdkDisplay
// yet, from a thread that may not be the one GTK ends up owning, leaves
// WebKit's cached platform display out of step with GTK's, which surfaces much
// later as "Error 71 (Protocol error) dispatching to Wayland display" once the
// webview surface is attached. The idle callback runs on the GTK main thread
// with the display already up, and spell checking is a plain context property,
// so applying it after the webview exists works the same.
func setupNativeSpellChecking() {
	C.enableWebKitSpellChecking()
}
