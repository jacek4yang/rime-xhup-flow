/* Real GTK surface probe: no Rime API, mock IME, or private user data. */
#include <gtk/gtk.h>
#include <glib/gstdio.h>

static const char *output;
static unsigned screenshot_count;

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
  g_free(text);
}

static void preedit(GtkTextView *view, gchar *text, gpointer unused) {
  (void)view;
  (void)unused;
  save_text("preedit.txt", text);
}

/* Screenshot requests never inject keys or alter the active composition. */
static gboolean capture(gpointer unused) {
  (void)unused;
  gchar *request = g_build_filename(output, "capture.request", NULL);
  gboolean requested = g_file_test(request, G_FILE_TEST_EXISTS);
  if (requested && g_unlink(request) != 0) g_error("could not consume capture request");
  g_free(request);
  if (!requested) return G_SOURCE_CONTINUE;
  gdk_display_sync(gdk_display_get_default());
  GdkWindow *root = gdk_get_default_root_window();
  GdkPixbuf *pixels = gdk_pixbuf_get_from_window(root, 0, 0,
      gdk_window_get_width(root), gdk_window_get_height(root));
  if (!pixels) g_error("could not capture actual X11 surface");
  gchar *path = g_strdup_printf("%s/screen-%u.png", output, screenshot_count++);
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
  GtkWidget *window = gtk_window_new(GTK_WINDOW_TOPLEVEL);
  gtk_window_set_title(GTK_WINDOW(window), "XHUP isolated Fcitx input probe");
  gtk_window_set_default_size(GTK_WINDOW(window), 800, 300);
  GtkWidget *view = gtk_text_view_new();
  gtk_text_view_set_left_margin(GTK_TEXT_VIEW(view), 16);
  gtk_text_view_set_top_margin(GTK_TEXT_VIEW(view), 16);
  GtkTextBuffer *buffer = gtk_text_view_get_buffer(GTK_TEXT_VIEW(view));
  g_signal_connect(buffer, "changed", G_CALLBACK(changed), NULL);
  g_signal_connect(view, "preedit-changed", G_CALLBACK(preedit), NULL);
  g_timeout_add(100, capture, NULL);
  g_signal_connect(window, "destroy", G_CALLBACK(gtk_main_quit), NULL);
  gtk_container_add(GTK_CONTAINER(window), view);
  gtk_widget_show_all(window);
  gtk_widget_grab_focus(view);
  changed(buffer, NULL);
  save_text("preedit.txt", "");
  g_print("GTK probe ready\n");
  gtk_main();
  return 0;
}
