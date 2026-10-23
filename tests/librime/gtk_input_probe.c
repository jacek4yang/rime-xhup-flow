/* Real GTK surface probe: no Rime API, mock IME, or private user data. */
#include <gtk/gtk.h>
#include <glib/gstdio.h>
#include <stdio.h>

static const char *output;
static FILE *trace;

static void trace_event(const char *kind, guint key, guint state, const char *text) {
  if (!trace) return;
  fprintf(trace, "%" G_GINT64_FORMAT "\t%" G_GINT64_FORMAT "\t%s\t%u\t%u\t%s\n",
          g_get_real_time(), g_get_monotonic_time(), kind, key, state, text ? text : "");
  fflush(trace);
}

static gboolean key_event(GtkWidget *widget, GdkEventKey *event, gpointer unused) {
  (void)widget;
  (void)unused;
  trace_event(event->type == GDK_KEY_PRESS ? "press" : "release",
              event->keyval, event->state, "");
  return FALSE;
}

static void save_text(const char *name, const char *text) {
  gchar *path = g_build_filename(output, name, NULL);
  GError *error = NULL;
  if (!g_file_set_contents(path, text, -1, &error)) g_error("%s", error->message);
  g_free(path);
}

static void changed(GtkTextBuffer *buffer, gpointer unused) {
  (void)unused;
  GtkTextIter start, end;
  gtk_text_buffer_get_bounds(buffer, &start, &end);
  gchar *text = gtk_text_buffer_get_text(buffer, &start, &end, FALSE);
  save_text("committed.txt", text);
  trace_event("committed", 0, 0, text);
  g_free(text);
}

static void preedit(GtkTextView *view, gchar *text, gpointer unused) {
  (void)view;
  (void)unused;
  trace_event("preedit", 0, 0, text);
  save_text("preedit.txt", text);
}

/* Screenshot requests never inject keys or alter the active composition. */
static gboolean capture(gpointer unused) {
  (void)unused;
  gchar *request = g_build_filename(output, "capture.request", NULL);
  if (!g_file_test(request, G_FILE_TEST_EXISTS)) {
    g_free(request);
    return G_SOURCE_CONTINUE;
  }
  gchar *contents = NULL, *end = NULL;
  if (!g_file_get_contents(request, &contents, NULL, NULL)) g_error("could not read capture request");
  guint64 index = g_ascii_strtoull(contents, &end, 10);
  if (end == contents || *end || index > G_MAXUINT) g_error("invalid capture index");
  if (g_unlink(request) != 0) g_error("could not consume capture request");
  g_free(contents);
  g_free(request);
  gdk_display_sync(gdk_display_get_default());
  GdkWindow *root = gdk_get_default_root_window();
  GdkPixbuf *pixels = gdk_pixbuf_get_from_window(root, 0, 0,
      gdk_window_get_width(root), gdk_window_get_height(root));
  if (!pixels) g_error("could not capture actual X11 surface");
  gchar *path = g_strdup_printf("%s/screen-%u.png", output, (guint)index);
  GError *error = NULL;
  if (!gdk_pixbuf_save(pixels, path, "png", &error, NULL)) g_error("%s", error->message);
  g_free(path);
  g_object_unref(pixels);
  return G_SOURCE_CONTINUE;
}

int main(int argc, char **argv) {
  gtk_init(&argc, &argv);
  if (argc != 2) return 2;
  output = argv[1];
  if (g_getenv("XHUP_TRACE_INPUT")) {
    gchar *path = g_build_filename(output, "input-events.tsv", NULL);
    trace = fopen(path, "w");
    g_free(path);
    if (!trace) g_error("could not open isolated input trace");
  }
  GtkWidget *window = gtk_window_new(GTK_WINDOW_TOPLEVEL);
  gtk_window_set_title(GTK_WINDOW(window), "XHUP isolated Fcitx input probe");
  gtk_window_set_default_size(GTK_WINDOW(window), 800, 300);
  GtkWidget *view = gtk_text_view_new();
  gtk_text_view_set_left_margin(GTK_TEXT_VIEW(view), 16);
  gtk_text_view_set_top_margin(GTK_TEXT_VIEW(view), 16);
  GtkTextBuffer *buffer = gtk_text_view_get_buffer(GTK_TEXT_VIEW(view));
  g_signal_connect(buffer, "changed", G_CALLBACK(changed), NULL);
  g_signal_connect(view, "preedit-changed", G_CALLBACK(preedit), NULL);
  g_signal_connect(view, "key-press-event", G_CALLBACK(key_event), NULL);
  g_signal_connect(view, "key-release-event", G_CALLBACK(key_event), NULL);
  g_timeout_add(100, capture, NULL);
  g_signal_connect(window, "destroy", G_CALLBACK(gtk_main_quit), NULL);
  gtk_container_add(GTK_CONTAINER(window), view);
  gtk_widget_show_all(window);
  gtk_widget_grab_focus(view);
  changed(buffer, NULL);
  save_text("preedit.txt", "");
  g_print("GTK probe ready\n");
  gtk_main();
  if (trace) fclose(trace);
  return 0;
}
