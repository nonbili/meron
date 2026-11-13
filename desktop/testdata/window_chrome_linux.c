// Appended to window_chrome_linux.go's C preamble by the native startup test.
// Each process owns a fresh GTK display and window, just like an app launch.
void goWindowStateChanged(void) {}
static gboolean fallbackReported = FALSE;
void goWindowTitlebarUnavailable(void) { fallbackReported = TRUE; }

int main(int argc, char **argv) {
	gboolean integrated = argc > 1;
	gboolean missedHook = integrated && g_strcmp0(argv[1], "missed-hook") == 0;
	gboolean background = integrated && g_strcmp0(argv[1], "background") == 0;
	gulong styleHook = installWindowChrome(integrated);
	if (missedHook) {
		g_signal_remove_emission_hook(g_signal_lookup("style-updated", GTK_TYPE_WIDGET), styleHook);
	}
	if (!gtk_init_check(&argc, &argv)) return 77;
	GtkWidget *window = gtk_window_new(GTK_WINDOW_TOPLEVEL);
	gtk_container_add(GTK_CONTAINER(window), gtk_label_new("Titlebar startup test"));
	if (background) {
		prepareBackgroundWindow(window);
		g_assert(mainWindow == GTK_WINDOW(window));
		g_assert(!gtk_widget_get_mapped(window));
		g_assert(!startupRevealed);
		maximiseOnNextConfigure();
		onBackgroundConfigure(window, NULL, NULL);
		g_assert(maximiseAfterConfigure);
		revealStartupWindow(TRUE);
		while (gtk_events_pending()) gtk_main_iteration();
		g_assert(startupRevealed);
		g_assert(!gtk_widget_get_mapped(window));
	}
	gtk_widget_show_all(window);
	while (gtk_events_pending()) gtk_main_iteration();
	if (background) {
		gint64 deadline = g_get_monotonic_time() + G_TIME_SPAN_SECOND;
		while (maximiseAfterConfigure && g_get_monotonic_time() < deadline) {
			while (gtk_events_pending()) gtk_main_iteration();
			g_usleep(1000);
		}
		g_assert(!maximiseAfterConfigure);
	}

	g_assert(mainWindow == GTK_WINDOW(window));
	g_assert(gtk_widget_get_realized(window));
	g_assert(gtk_widget_get_mapped(window));
	if (missedHook && g_strcmp0(g_getenv("GDK_BACKEND"), "x11") == 0) g_assert(fallbackReported);
	if (missedHook && fallbackReported) {
		g_assert(!wantIntegrated);
		g_assert(gtk_window_get_titlebar(mainWindow) == NULL);
		gtk_widget_destroy(window);
		return 0;
	}
	g_assert(!fallbackReported);
	g_assert(wantIntegrated == integrated);
	if (g_strcmp0(g_getenv("GDK_BACKEND"), "x11") == 0) {
		g_assert(!switchesTitlebarLive());
	}
	GtkWidget *bar = gtk_window_get_titlebar(mainWindow);
	if (integrated) {
		g_assert(bar != NULL);
		g_assert(!gtk_widget_get_visible(bar));
		applyTitlebar();
		g_assert(gtk_window_get_titlebar(mainWindow) == bar);
	} else {
		// A native GTK header is internal; KDE's native header is external.
		// Neither is exposed by get_titlebar as a custom titlebar.
		g_assert(bar == NULL);
	}

	if (switchesTitlebarLive()) {
		wantIntegrated = TRUE;
		applyTitlebar();
		wantIntegrated = FALSE;
		applyTitlebar();
		g_assert(GTK_IS_HEADER_BAR(gtk_window_get_titlebar(mainWindow)));
		g_assert(gtk_widget_get_visible(gtk_window_get_titlebar(mainWindow)));
		wantIntegrated = TRUE;
		applyTitlebar();
		g_assert(!gtk_widget_get_visible(gtk_window_get_titlebar(mainWindow)));
	}
	gtk_widget_destroy(window);
	return 0;
}
