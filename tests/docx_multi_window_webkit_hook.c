/* External-only WebKit probe hook for the synthetic two-window DOCX test. */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <webkit2/webkit2.h>
#include <stdio.h>
#include <stdlib.h>

static int next_index = 0;

static void progress_completed(GObject *object, GAsyncResult *result, gpointer unused) {
    (void)unused;
    GError *error = NULL;
    JSCValue *value = webkit_web_view_evaluate_javascript_finish(WEBKIT_WEB_VIEW(object), result, &error);
    if (value) {
        char *text = jsc_value_to_string(value);
        int index = GPOINTER_TO_INT(g_object_get_data(object, "docx-window-index"));
        const char *base = getenv("NOVEL_RELEASE_TEST_RESULT");
        if (base) {
            char path[4096];
            snprintf(path, sizeof(path), "%s.progress.%d", base, index);
            FILE *file = fopen(path, "w");
            if (file) { fputs(text, file); fclose(file); }
        }
        g_printerr("DOCX window %d progress %s\n", index, text);
        g_free(text);
        g_object_unref(value);
    }
    if (error) g_error_free(error);
}

static gboolean progress_tick(gpointer object) {
    const char *script = "JSON.stringify({stage:(window.__novelReleaseProgress&&window.__novelReleaseProgress.stage)||'hook-loaded',windowLabel:(window.__novelReleaseProgress&&window.__novelReleaseProgress.windowLabel)||'',errors:[...document.querySelectorAll('[role=alert]')].map(e=>e.textContent),buttons:[...document.querySelectorAll('.window-workspace > [role=tabpanel]:not([hidden]) button')].map(e=>e.textContent.trim()).filter(Boolean).slice(-30),tabs:[...document.querySelectorAll('[role=tab]')].map(e=>({text:e.textContent,selected:e.getAttribute('aria-selected')}))})";
    webkit_web_view_evaluate_javascript(WEBKIT_WEB_VIEW(object), script, -1, NULL, NULL, NULL, progress_completed, NULL);
    return G_SOURCE_CONTINUE;
}

static void completed(GObject *object, GAsyncResult *result, gpointer unused) {
    (void)unused;
    GError *error = NULL;
    JSCValue *value = webkit_web_view_call_async_javascript_function_finish(WEBKIT_WEB_VIEW(object), result, &error);
    const char *base = getenv("NOVEL_RELEASE_TEST_RESULT");
    int index = GPOINTER_TO_INT(g_object_get_data(object, "docx-window-index"));
    if (base) {
        char path[4096];
        snprintf(path, sizeof(path), "%s.%d", base, index);
        FILE *file = fopen(path, "w");
        if (file) {
            if (value) { char *text = jsc_value_to_string(value); fputs(text, file); g_free(text); }
            else fprintf(file, "{\"error\":\"External WebKit evaluation failed: %s\"}", error ? error->message : "unknown");
            fclose(file);
        }
    }
    if (value) g_object_unref(value);
    if (error) g_error_free(error);
}

static void loaded(WebKitWebView *view, WebKitLoadEvent event, gpointer unused) {
    (void)unused;
    if (event != WEBKIT_LOAD_FINISHED || g_object_get_data(G_OBJECT(view), "docx-probe-ran")) return;
    g_object_set_data(G_OBJECT(view), "docx-probe-ran", GINT_TO_POINTER(1));
    g_timeout_add(400, progress_tick, view);
    const char *path = getenv("NOVEL_RELEASE_TEST_SCRIPT");
    char *script = NULL;
    if (!path || !g_file_get_contents(path, &script, NULL, NULL)) return;
    webkit_web_view_call_async_javascript_function(view, script, -1, NULL, NULL, NULL, NULL, completed, NULL);
    g_free(script);
}

static void register_view(WebKitWebView *view) {
    if (g_object_get_data(G_OBJECT(view), "docx-probe-hook")) return;
    g_object_set_data(G_OBJECT(view), "docx-probe-hook", GINT_TO_POINTER(1));
    g_object_set_data(G_OBJECT(view), "docx-window-index", GINT_TO_POINTER(next_index++));
    g_signal_connect(view, "load-changed", G_CALLBACK(loaded), NULL);
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
