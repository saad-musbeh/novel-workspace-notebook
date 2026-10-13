/* Test-only instrumentation for an extracted release in isolated XDG folders.
 * This shared object is never linked into or shipped with the application. */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <stdio.h>
#include <stdlib.h>
#include <webkit2/webkit2.h>

static void snapshot_completed(GObject *object, GAsyncResult *result, gpointer ignored) {
    GError *error = NULL;
    cairo_surface_t *surface = webkit_web_view_get_snapshot_finish(WEBKIT_WEB_VIEW(object), result, &error);
    const char *path = getenv("NOVEL_RELEASE_TEST_SCREENSHOT");
    if (surface) { if (path) cairo_surface_write_to_png(surface, path); cairo_surface_destroy(surface); }
    if (error) g_error_free(error);
}
static void completed(GObject *object, GAsyncResult *result, gpointer ignored) {
    GError *error = NULL;
    JSCValue *value = webkit_web_view_call_async_javascript_function_finish(WEBKIT_WEB_VIEW(object), result, &error);
    if (getenv("NOVEL_PROBE_DIAGNOSTICS"))
        g_printerr("Novel hook completed index=%d at=%lld value=%d error=%s\n", GPOINTER_TO_INT(g_object_get_data(object, "novel-window-index"))-1, (long long)g_get_monotonic_time(), value != NULL, error ? error->message : "none");
    const char *path = getenv("NOVEL_RELEASE_TEST_RESULT");
    if (path) {
        char report_path[4096];
        int index = GPOINTER_TO_INT(g_object_get_data(object, "novel-window-index")) - 1;
        snprintf(report_path, sizeof(report_path), "%s.%d", path, index);
        FILE *output = fopen(report_path, "w");
        if (output) {
            if (value) { char *text = jsc_value_to_string(value); fputs(text, output); g_free(text); }
            else fputs("{\"error\":\"Release JavaScript evaluation failed\"}", output);
            fclose(output);
        }
    }
    if (getenv("NOVEL_RELEASE_TEST_SCREENSHOT"))
        webkit_web_view_get_snapshot(WEBKIT_WEB_VIEW(object), WEBKIT_SNAPSHOT_REGION_VISIBLE,
            WEBKIT_SNAPSHOT_OPTIONS_NONE, NULL, snapshot_completed, NULL);
    if (value) g_object_unref(value);
    if (error) g_error_free(error);
}
static void window_destroyed(GtkWidget *window, gpointer user_data) {
    (void)window;
    const char *path = getenv("NOVEL_WINDOW_DESTROY_LOG");
    if (!path) return;
    FILE *output = fopen(path, "a");
    if (output) {fprintf(output, "%d %lld\n", GPOINTER_TO_INT(user_data) - 1, (long long)g_get_monotonic_time()); fclose(output);}
}
static void loaded(WebKitWebView *view, WebKitLoadEvent event, gpointer ignored) {
    if (getenv("NOVEL_PROBE_DIAGNOSTICS"))
        g_printerr("Novel hook loaded index=%d event=%d at=%lld\n", GPOINTER_TO_INT(g_object_get_data(G_OBJECT(view), "novel-window-index"))-1, event, (long long)g_get_monotonic_time());
    if (event != WEBKIT_LOAD_FINISHED || g_object_get_data(G_OBJECT(view), "novel-release-test-ran")) return;
    GtkWidget *window = gtk_widget_get_toplevel(GTK_WIDGET(view));
    if (GTK_IS_APPLICATION_WINDOW(window) && !g_object_get_data(G_OBJECT(window), "novel-destroy-hook")) {
        g_object_set_data(G_OBJECT(window), "novel-destroy-hook", GINT_TO_POINTER(1));
        g_signal_connect(window, "destroy", G_CALLBACK(window_destroyed), g_object_get_data(G_OBJECT(view), "novel-window-index"));
    }
    const char *path = getenv("NOVEL_RELEASE_TEST_SCRIPT");
    char *script = NULL;
    if (!path || !g_file_get_contents(path, &script, NULL, NULL)) return;
    g_object_set_data(G_OBJECT(view), "novel-release-test-ran", GINT_TO_POINTER(1));
    if (getenv("NOVEL_PROBE_DIAGNOSTICS"))
        g_printerr("Novel hook dispatch index=%d at=%lld\n", GPOINTER_TO_INT(g_object_get_data(G_OBJECT(view), "novel-window-index"))-1, (long long)g_get_monotonic_time());
    webkit_web_view_call_async_javascript_function(view, script, -1, NULL, NULL, NULL, NULL, completed, NULL);
    g_free(script);
}
static int next_window_index = 1;
static void register_view(WebKitWebView *view) {
    if (!g_object_get_data(G_OBJECT(view), "novel-release-test-hook")) {
        g_object_set_data(G_OBJECT(view), "novel-release-test-hook", GINT_TO_POINTER(1));
        g_object_set_data(G_OBJECT(view), "novel-window-index", GINT_TO_POINTER(next_window_index++));
        g_signal_connect(view, "load-changed", G_CALLBACK(loaded), NULL);
    }
}
void webkit_web_view_load_uri(WebKitWebView *view, const char *uri) {
    register_view(view);
    void (*original)(WebKitWebView *, const char *) = dlsym(RTLD_NEXT, "webkit_web_view_load_uri");
    original(view, uri);
}
void webkit_web_view_load_html(WebKitWebView *view, const char *content, const char *base_uri) {
    register_view(view);
    void (*original)(WebKitWebView *, const char *, const char *) = dlsym(RTLD_NEXT, "webkit_web_view_load_html");
    original(view, content, base_uri);
}
