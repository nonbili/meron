//go:build linux && webkit2_41

package main

/*
#cgo pkg-config: webkit2gtk-4.1

#include <glib.h>
#include <webkit2/webkit2.h>

static void enableWebKitSpellChecking() {
	WebKitWebContext *context = webkit_web_context_get_default();
	webkit_web_context_set_spell_checking_languages(context, g_get_language_names());
	webkit_web_context_set_spell_checking_enabled(context, TRUE);
}
*/
import "C"

func setupNativeSpellChecking() {
	C.enableWebKitSpellChecking()
}
