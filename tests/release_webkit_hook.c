/* Test-only instrumentation for an extracted release in isolated XDG folders.
 * This shared object is never linked into or shipped with the application. */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <stdio.h>
#include <stdlib.h>
#include <webkit2/webkit2.h>

static void progress_completed(GObject *object, GAsyncResult *result, gpointer ignored) {
    GError *error = NULL;
    JSCValue *value = webkit_web_view_evaluate_javascript_finish(WEBKIT_WEB_VIEW(object), result, &error);
    const char *path = getenv("NOVEL_RELEASE_TEST_PROGRESS");
    if (path && value) {
        FILE *output = fopen(path, "w");
        if (output) { char *text = jsc_value_to_string(value); fputs(text, output); g_free(text); fclose(output); }
    }
    if (value) g_object_unref(value);
    if (error) g_error_free(error);
}
static gboolean progress_tick(gpointer object) {
    const char *script = "JSON.stringify({report:window.__novelReleaseProgress,alerts:[...document.querySelectorAll('[role=alert]')].map(e=>e.textContent),heading:[...document.querySelectorAll('.topbar h1')].map(e=>({text:e.textContent,visible:Boolean(e.getClientRects().length)}))})";
    webkit_web_view_evaluate_javascript(WEBKIT_WEB_VIEW(object), script, -1, NULL, NULL, NULL, progress_completed, NULL);
    return G_SOURCE_CONTINUE;
}

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
    const char *path = getenv("NOVEL_RELEASE_TEST_RESULT");
    if (path) {
        FILE *output = fopen(path, "w");
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
static void loaded(WebKitWebView *view, WebKitLoadEvent event, gpointer ignored) {
    if (event != WEBKIT_LOAD_FINISHED || g_object_get_data(G_OBJECT(view), "novel-release-test-ran")) return;
    const char *path = getenv("NOVEL_RELEASE_TEST_SCRIPT");
    char *script = NULL;
    if (!path || !g_file_get_contents(path, &script, NULL, NULL)) return;
    g_object_set_data(G_OBJECT(view), "novel-release-test-ran", GINT_TO_POINTER(1));
    if (getenv("NOVEL_RELEASE_TEST_PROGRESS"))
        g_timeout_add_full(G_PRIORITY_DEFAULT, 1000, progress_tick, g_object_ref(view), g_object_unref);
    webkit_web_view_call_async_javascript_function(view, script, -1, NULL, NULL, NULL, NULL, completed, NULL);
    g_free(script);
}
static void register_view(WebKitWebView *view) {
    if (!g_object_get_data(G_OBJECT(view), "novel-release-test-hook")) {
        g_object_set_data(G_OBJECT(view), "novel-release-test-hook", GINT_TO_POINTER(1));
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
