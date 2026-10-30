/* Real file-monitor and JSON parsing, plus frame interpolation without a display. */
#include "workspace-animation.c"
#include <glib/gstdio.h>

static void test_frames(void) {
    for (guint hz = 60; hz <= 240; hz *= 2) {
        for (guint closing = 0; closing < 2; closing++) {
            double from = closing ? 620 : 330, target = closing ? 330 : 620;
            double previous = from;
            for (gint64 elapsed = 0; elapsed <= 250000; elapsed += 1000000 / hz) {
                double width = frame_width(from, target, elapsed);
                g_assert_cmpfloat(width, >=, 330);
                g_assert_cmpfloat(width, <=, 620);
                if (closing) g_assert_cmpfloat(width, <=, previous);
                else g_assert_cmpfloat(width, >=, previous);
                previous = width;
            }
            g_assert_cmpfloat(previous, ==, target);
        }
    }
    double mid = frame_width(330, 620, 80000);
    g_assert_cmpfloat(frame_width(mid, 330, 0), ==, mid);
    g_assert_cmpfloat(frame_width(mid, 330, 220000), ==, 330);
    g_print("Frame clock interpolation: opening, closing and reversal passed\n");
}

int main(void) {
    const char *icons[] = {"1", "2", "3", "4", "5", "6", "α", "β", "γ", "δ", "ε", "ζ"};
    for (guint i = 0; i < G_N_ELEMENTS(icons); i++) {
        char *label = g_strdup_printf("%s 󰈹 󰨞", icons[i]);
        g_assert_cmpint(workspace_id_from_label(label), ==, (int)i + 1);
        g_free(label);
    }
    g_assert_cmpint(workspace_id_from_label("13 󰈹"), ==, 13);
    g_assert_cmpint(workspace_id_from_label("42 "), ==, 42);
    const char *invalid[] = {NULL, "", " 1", "󰈹 1", "0", "-1", "special", "1x", "αβ", "999999999999999999999999"};
    for (guint i = 0; i < G_N_ELEMENTS(invalid); i++)
        g_assert_cmpint(workspace_id_from_label(invalid[i]), ==, 0);
    g_print("Workspace click targets: numeric/Greek icons and invalid labels passed\n");
    char *directory = g_dir_make_tmp("waybar-animation-test-XXXXXX", NULL);
    g_assert_nonnull(directory);
    char *path = g_build_filename(directory, "state.json", NULL);
    char *temporary = g_strconcat(path, ".tmp", NULL);
    Animation animation = {.state_path = path};
    GFile *folder = g_file_new_for_path(directory);
    GFileMonitor *monitor = g_file_monitor_directory(folder, G_FILE_MONITOR_WATCH_MOVES, NULL, NULL);
    g_assert_nonnull(monitor);
    g_file_monitor_set_rate_limit(monitor, 0);
    g_signal_connect(monitor, "changed", G_CALLBACK(state_changed), &animation);
    const char *states[] = {
        "{\"expanded\":false}",
        "{\"expanded\":true}",
        "{\"expanded\":false}",
        "{\"mode\":\"pinned\",\"active\":1,\"occupied\":false}",
        "{\"mode\":\"hidden\",\"active\":7,\"occupied\":true}",
        "{\"mode\":\"auto\",\"active\":7,\"occupied\":false}",
    };
    for (guint i = 0; i < G_N_ELEMENTS(states); i++) {
        animation.state_known = FALSE;
        g_assert_true(g_file_set_contents(temporary, states[i], -1, NULL));
        g_assert_cmpint(g_rename(temporary, path), ==, 0);
        gint64 deadline = g_get_monotonic_time() + 500000;
        while (!animation.state_known && g_get_monotonic_time() < deadline) {
            while (g_main_context_iteration(NULL, FALSE)) {}
            g_usleep(1000);
        }
        g_assert_true(animation.state_known);
        g_assert_cmpint(animation.expanded, ==, i % 2);
    }
    g_file_monitor_cancel(monitor);
    g_object_unref(monitor);
    g_object_unref(folder);
    g_remove(path);
    g_rmdir(directory);
    g_free(temporary); g_free(path); g_free(directory);
    g_print("Native state monitor: six open/close transitions passed\n");
    test_frames();
    return 0;
}
