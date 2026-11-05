//go:build linux && gtk3 && !bindings

package main

/*
#cgo pkg-config: webkit2gtk-4.1
#include <webkit2/webkit2.h>
#include <stdlib.h>

typedef struct {
    GMutex mutex;
    GCond condition;
    char *html;
    gboolean done;
    int success;
} MailPrintJob;

typedef struct {
    gboolean done;
    gboolean success;
    int references;
} MailPrintResult;

static void releaseMailPrintResult(MailPrintResult *result) {
    if (--result->references == 0) g_free(result);
}

static WebKitWebView *findMailWebView(GtkWidget *widget) {
    if (WEBKIT_IS_WEB_VIEW(widget)) return WEBKIT_WEB_VIEW(widget);
    if (!GTK_IS_CONTAINER(widget)) return NULL;
    GList *children = gtk_container_get_children(GTK_CONTAINER(widget));
    WebKitWebView *view = NULL;
    for (GList *child = children; child && !view; child = child->next)
        view = findMailWebView(GTK_WIDGET(child->data));
    g_list_free(children);
    return view;
}

static void mailPrintEvaluated(GObject *source, GAsyncResult *async, gpointer data) {
    MailPrintResult *result = data;
    GError *error = NULL;
    JSCValue *value = webkit_web_view_evaluate_javascript_finish(WEBKIT_WEB_VIEW(source), async, &error);
    result->success = !error && value && jsc_value_to_boolean(value);
    if (value) g_object_unref(value);
    g_clear_error(&error);
    result->done = TRUE;
    releaseMailPrintResult(result);
}

// Callback state is heap-owned: cancellation may complete after the deadline.
static gboolean evaluateMailPrint(WebKitWebView *view, const char *script, gint64 deadline) {
    MailPrintResult *result = g_new0(MailPrintResult, 1);
    result->references = 2;
    GCancellable *cancel = g_cancellable_new();
    webkit_web_view_evaluate_javascript(view, script, -1, NULL, NULL, cancel, mailPrintEvaluated, result);
    while (!result->done && g_get_monotonic_time() < deadline) {
        g_main_context_iteration(NULL, FALSE);
        g_usleep(1000);
    }
    gboolean success = result->done && result->success;
    if (!result->done) g_cancellable_cancel(cancel);
    g_object_unref(cancel);
    releaseMailPrintResult(result);
    return success;
}

static void mailPrintFinished(WebKitPrintOperation *operation, gpointer data) {
    ((MailPrintResult *)data)->done = TRUE;
}

static void mailPrintFailed(WebKitPrintOperation *operation, GError *error, gpointer data) {
    ((MailPrintResult *)data)->success = FALSE;
}

static gboolean runMailPrint(gpointer data) {
    MailPrintJob *job = data;
    GtkWindow *parent = NULL;
    WebKitWebView *source = NULL;
    GList *windows = gtk_window_list_toplevels();
    for (GList *window = windows; window && !source; window = window->next) {
        source = findMailWebView(GTK_WIDGET(window->data));
        if (source) parent = GTK_WINDOW(window->data);
    }
    g_list_free(windows);
    if (source) {
        // Use Wails' web context (including its local-media URI handler), but
        // a new content manager so app scripts are not injected into the mail.
        WebKitWebView *view = WEBKIT_WEB_VIEW(webkit_web_view_new_with_context(webkit_web_view_get_context(source)));
        g_object_ref_sink(view);
        webkit_web_view_load_html(view, job->html, webkit_web_view_get_uri(source));
        gint64 deadline = g_get_monotonic_time() + 30 * G_TIME_SPAN_SECOND;
        while (webkit_web_view_is_loading(view) && g_get_monotonic_time() < deadline) {
            g_main_context_iteration(NULL, FALSE);
            g_usleep(1000);
        }
        job->success = -2; // The print document did not finish loading.
        if (!webkit_web_view_is_loading(view)) {
            job->success = -3; // The message templates could not be attached.
            const char *prepare = "(() => { for (const t of document.querySelectorAll('template[data-print-message]')) { const mail = new DOMParser().parseFromString(t.content.textContent, 'text/html'); t.parentElement.attachShadow({mode:'open'}).append(document.importNode(mail.documentElement, true)); t.remove(); } return true; })()";
            if (evaluateMailPrint(view, prepare, g_get_monotonic_time() + 5 * G_TIME_SPAN_SECOND)) {
                // Template images load after the shadow roots are attached.
                deadline = g_get_monotonic_time() + 15 * G_TIME_SPAN_SECOND;
                do {
                    gboolean imagesReady = evaluateMailPrint(view,
                        "Array.from(document.querySelectorAll('[data-print-body]')).every(s => s.shadowRoot && Array.from(s.shadowRoot.querySelectorAll('img')).every(i => i.complete))",
                        MIN(deadline, g_get_monotonic_time() + 5 * G_TIME_SPAN_SECOND));
                    if (imagesReady) break;
                    g_main_context_iteration(NULL, FALSE);
                    g_usleep(10000);
                } while (g_get_monotonic_time() < deadline);
                WebKitPrintOperation *operation = webkit_print_operation_new(view);
                GtkPageSetup *setup = gtk_page_setup_new();
                gtk_page_setup_set_top_margin(setup, 15, GTK_UNIT_MM);
                gtk_page_setup_set_bottom_margin(setup, 15, GTK_UNIT_MM);
                gtk_page_setup_set_left_margin(setup, 15, GTK_UNIT_MM);
                gtk_page_setup_set_right_margin(setup, 15, GTK_UNIT_MM);
                webkit_print_operation_set_page_setup(operation, setup);
                g_object_unref(setup);
                MailPrintResult printed = {FALSE, TRUE};
                g_signal_connect(operation, "finished", G_CALLBACK(mailPrintFinished), &printed);
                g_signal_connect(operation, "failed", G_CALLBACK(mailPrintFailed), &printed);
                if (webkit_print_operation_run_dialog(operation, parent) == WEBKIT_PRINT_OPERATION_RESPONSE_PRINT)
                    while (!printed.done) g_main_context_iteration(NULL, TRUE);
                job->success = printed.success;
                g_signal_handlers_disconnect_by_data(operation, &printed);
                g_object_unref(operation);
            }
        }
        webkit_web_view_stop_loading(view);
        gtk_widget_destroy(GTK_WIDGET(view));
        g_object_unref(view);
    }
    g_mutex_lock(&job->mutex);
    job->done = TRUE;
    g_cond_signal(&job->condition);
    g_mutex_unlock(&job->mutex);
    return G_SOURCE_REMOVE;
}

static int printLinuxMail(char *html) {
    MailPrintJob job = {0};
    job.success = -1; // No app web view. Negative results are safe to fall back from.
    job.html = html;
    g_mutex_init(&job.mutex);
    g_cond_init(&job.condition);
    if (g_main_context_is_owner(g_main_context_default())) {
        runMailPrint(&job);
    } else {
        g_mutex_lock(&job.mutex);
        g_idle_add(runMailPrint, &job);
        while (!job.done) g_cond_wait(&job.condition, &job.mutex);
        g_mutex_unlock(&job.mutex);
    }
    g_cond_clear(&job.condition);
    g_mutex_clear(&job.mutex);
    return job.success;
}
*/
import "C"

import (
	"fmt"
	"unsafe"
)

var nativePrintFallbacks = map[C.int]string{
	-1: "no app web view",
	-2: "print document did not load",
	-3: "could not attach message templates",
}

// printNativeMail reports why it declined, so the browser fallback is traceable.
func printNativeMail(html string) (bool, string, error) {
	document := C.CString(html)
	defer C.free(unsafe.Pointer(document))
	result := C.printLinuxMail(document)
	if result < 0 {
		return false, nativePrintFallbacks[result], nil
	}
	if result == 0 {
		return false, "", fmt.Errorf("could not print mail document")
	}
	return true, "", nil
}
