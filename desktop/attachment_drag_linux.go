//go:build linux && !bindings

package main

/*
#cgo pkg-config: gtk+-3.0
#include <gtk/gtk.h>
#include <stdlib.h>

static void attachmentDragData(GtkWidget *widget, GdkDragContext *context,
                               GtkSelectionData *data, guint info, guint time, gpointer uri) {
	char *uris[] = {uri, NULL};
	gtk_selection_data_set_uris(data, uris);
}

static void freeAttachmentDragURI(gpointer uri, GClosure *closure) {
	g_free(uri);
}

static void attachmentDragEnd(GtkWidget *widget, GdkDragContext *context, gpointer unused) {
	g_signal_handlers_disconnect_matched(widget, G_SIGNAL_MATCH_FUNC, 0, 0, NULL, attachmentDragData, NULL);
	g_signal_handlers_disconnect_by_func(widget, attachmentDragEnd, NULL);
}

static gboolean startAttachmentDrag(const char *path) {
	GtkWidget *source = NULL;
	GList *windows = gtk_window_list_toplevels();
	for (GList *entry = windows; entry; entry = entry->next) {
		if (gtk_window_is_active(GTK_WINDOW(entry->data))) {
			source = GTK_WIDGET(entry->data);
			break;
		}
	}
	g_list_free(windows);
	if (!source || !gtk_widget_get_realized(source)) return FALSE;

	GdkWindow *window = gtk_widget_get_window(source);
	GdkSeat *seat = gdk_display_get_default_seat(gtk_widget_get_display(source));
	GdkDevice *pointer = seat ? gdk_seat_get_pointer(seat) : NULL;
	if (!pointer) return FALSE;
	GdkModifierType state = 0;
	gdk_window_get_device_position(window, pointer, NULL, NULL, &state);
	if (!(state & GDK_BUTTON1_MASK)) return FALSE;

	char *uri = g_filename_to_uri(path, NULL, NULL);
	if (!uri) return FALSE;

	GtkTargetList *targets = gtk_target_list_new(NULL, 0);
	gtk_target_list_add_uri_targets(targets, 0);
	GdkDragContext *drag = gtk_drag_begin_with_coordinates(source, targets, GDK_ACTION_COPY, 1, NULL, -1, -1);
	gtk_target_list_unref(targets);
	if (!drag) {
		g_free(uri);
		return FALSE;
	}
	g_signal_connect_data(source, "drag-data-get", G_CALLBACK(attachmentDragData), uri, freeAttachmentDragURI, 0);
	g_signal_connect(source, "drag-end", G_CALLBACK(attachmentDragEnd), NULL);
	gtk_drag_set_icon_name(drag, "text-x-generic", 0, 0);
	return TRUE;
}
*/
import "C"

import (
	"unsafe"

	"github.com/wailsapp/wails/v3/pkg/application"
)

func startNativeAttachmentDrag(path string) (bool, error) {
	name := C.CString(path)
	defer C.free(unsafe.Pointer(name))
	var started bool
	application.InvokeSync(func() { started = C.startAttachmentDrag(name) != 0 })
	return started, nil
}
