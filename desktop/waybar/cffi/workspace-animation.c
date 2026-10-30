/* Clip the single native workspace list at its normal, cached text size.
 * Only the viewport width changes, synchronized to GTK's frame clock.
 * Waybar CFFI ABI 2:
 * https://github.com/Alexays/Waybar/blob/0.15.0/resources/custom_modules/cffi_example/waybar_cffi_module.h
 */
#include <gtk/gtk.h>
#include <json-glib/json-glib.h>
#include <math.h>

typedef struct wbcffi_module wbcffi_module;
typedef struct {
    wbcffi_module *obj;
    const char *waybar_version;
    GtkContainer *(*get_root_widget)(wbcffi_module *);
    void (*queue_update)(wbcffi_module *);
} wbcffi_init_info;
typedef struct { const char *key; const char *value; } wbcffi_config_entry;

const size_t wbcffi_version = 2;

typedef struct {
    GtkWidget *root;
    GtkWidget *workspaces;
    GtkWidget *viewport;
    GFileMonitor *monitor;
    char *state_path;
    guint attach_source;
    guint tick;
    gboolean expanded;
    gboolean state_known;
    int closed_width;
    int full_width;
    double width;
    double from;
    double target;
    gint64 started;
} Animation;

/* Waybar 0.15 sends the removed `dispatch workspace N` IPC request.
 * This config uses `{icon} {windows}` with numeric icons and alpha--zeta.
 * Read only that first token; app icons and titles must never select a target.
 */
static int workspace_id_from_label(const char *text) {
    if (!text) return 0;
    char **parts = g_strsplit_set(text, " \t\r\n", 2);
    const char *greek[] = {"α", "β", "γ", "δ", "ε", "ζ"};
    int id = 0;
    for (guint i = 0; i < G_N_ELEMENTS(greek); i++)
        if (g_strcmp0(parts[0], greek[i]) == 0) id = 7 + (int)i;
    if (!id && parts[0] && g_ascii_isdigit(parts[0][0])) {
        char *end = NULL;
        gint64 number = g_ascii_strtoll(parts[0], &end, 10);
        if (*end == '\0' && number > 0 && number <= G_MAXINT) id = (int)number;
    }
    g_strfreev(parts);
    return id;
}

static const char *workspace_label(GtkWidget *widget) {
    if (GTK_IS_LABEL(widget) && gtk_style_context_has_class(
            gtk_widget_get_style_context(widget), "workspace-label"))
        return gtk_label_get_text(GTK_LABEL(widget));
    if (!GTK_IS_CONTAINER(widget)) return NULL;
    GList *children = gtk_container_get_children(GTK_CONTAINER(widget));
    const char *text = NULL;
    for (GList *it = children; it && !text; it = it->next)
        text = workspace_label(it->data);
    g_list_free(children);
    return text;
}

static gboolean workspace_event(GtkWidget *widget, GdkEvent *event, gpointer data) {
    (void)data;
    if (event->type != GDK_BUTTON_PRESS) return FALSE;
    int id = workspace_id_from_label(workspace_label(widget));
    if (!id) return FALSE;
    char *expression = g_strdup_printf("hl.dispatch(hl.dsp.focus({ workspace = \"%d\" }))", id);
    char *argv[] = {"hyprctl", "eval", expression, NULL};
    GError *error = NULL;
    if (!g_spawn_async(NULL, argv, NULL, G_SPAWN_SEARCH_PATH, NULL, NULL, NULL, &error)) {
        g_warning("Cannot activate workspace %d: %s", id, error->message);
        g_error_free(error);
    }
    g_free(expression);
    /* ::event precedes ::button-press-event, preventing the legacy handler. */
    return TRUE;
}

static void connect_workspace_click(GtkWidget *child, gpointer data) {
    if (!GTK_IS_BUTTON(child)) return;
    if (!g_signal_handler_find(child, G_SIGNAL_MATCH_FUNC | G_SIGNAL_MATCH_DATA,
            0, 0, NULL, G_CALLBACK(workspace_event), data))
        g_signal_connect(child, "event", G_CALLBACK(workspace_event), data);
}

static void report_layout(Animation *animation, const char *phase) {
    if (!animation->state_path) return;
    char *directory = g_path_get_dirname(animation->state_path);
    char *path = g_build_filename(directory, "animation-layout.json", NULL);
    char *contents = g_strdup_printf(
        "{\"phase\":\"%s\",\"attached\":%s,\"expanded\":%s,"
        "\"closed_width\":%d,\"full_width\":%d,\"requested_width\":%.2f,\"allocated_width\":%d}\n",
        phase, animation->viewport ? "true" : "false", animation->expanded ? "true" : "false",
        animation->closed_width, animation->full_width, animation->width,
        animation->viewport ? gtk_widget_get_allocated_width(animation->viewport) : 0);
    g_file_set_contents(path, contents, -1, NULL);
    g_free(contents); g_free(path); g_free(directory);
}

static void viewport_allocated(GtkWidget *widget, GtkAllocation *allocation, gpointer data) {
    (void)widget; (void)allocation;
    Animation *animation = data;
    if (!animation->tick) report_layout(animation, "settled");
}

static double frame_width(double from, double target, gint64 elapsed) {
    double progress = CLAMP((double)elapsed / 220000.0, 0.0, 1.0);
    double remaining = 1.0 - progress;
    return from + (target - from) * (1.0 - remaining * remaining * remaining);
}

static gboolean animate_width(GtkWidget *widget, GdkFrameClock *clock, gpointer data) {
    Animation *animation = data;
    gint64 now = gdk_frame_clock_get_frame_time(clock);
    if (!animation->started) animation->started = now;
    animation->width = frame_width(animation->from, animation->target, now - animation->started);
    gtk_widget_set_size_request(widget, (int)lround(animation->width), -1);
    if (now - animation->started < 220000) return G_SOURCE_CONTINUE;
    animation->tick = 0;
    return G_SOURCE_REMOVE;
}

static void set_target(Animation *animation, gboolean animate) {
    if (!animation->viewport) return;
    double target = animation->expanded ? animation->full_width : animation->closed_width;
    if (animation->tick && target == animation->target && animate) return;
    if (animation->tick) {
        gtk_widget_remove_tick_callback(animation->viewport, animation->tick);
        animation->tick = 0;
    }
    animation->target = target;
    animation->from = animation->width;
    animation->started = 0;
    if (!animate || fabs(target - animation->width) < 0.5) {
        animation->width = target;
        gtk_widget_set_size_request(animation->viewport, (int)lround(target), -1);
    } else {
        animation->tick = gtk_widget_add_tick_callback(animation->viewport, animate_width, animation, NULL);
    }
}

static void measure_widths(Animation *animation) {
    if (!animation->workspaces) return;
    int full = 0, first_six = 0, all = 0, count = 0;
    gtk_widget_get_preferred_width(animation->workspaces, &full, NULL);
    GList *children = gtk_container_get_children(GTK_CONTAINER(animation->workspaces));
    for (GList *it = children; it; it = it->next) {
        GtkWidget *child = it->data;
        /* Native Waybar packs new buttons directly into its GtkBox. */
        connect_workspace_click(child, animation);
        if (!gtk_widget_get_visible(child)) continue;
        int width = 0;
        gtk_widget_get_preferred_width(child, &width, NULL);
        all += width;
        if (count++ < 6) first_six += width;
    }
    g_list_free(children);
    /* The native right padding follows the LAST workspace, not workspace 6.
     * Including it in the closed clip would expose the start of workspace 7. */
    GtkBorder padding = {0}, border = {0};
    GtkStyleContext *style = gtk_widget_get_style_context(animation->workspaces);
    GtkStateFlags flags = gtk_widget_get_state_flags(animation->workspaces);
    gtk_style_context_get_padding(style, flags, &padding);
    gtk_style_context_get_border(style, flags, &border);
    int leading = MAX(0, full - all - padding.right - border.right);
    animation->closed_width = count > 6 ? MAX(1, first_six + leading) : MAX(1, full);
    animation->full_width = MAX(animation->closed_width, full);
}

static void workspaces_allocated(GtkWidget *widget, GtkAllocation *allocation, gpointer data) {
    (void)widget; (void)allocation;
    Animation *animation = data;
    int old_closed = animation->closed_width, old_full = animation->full_width;
    measure_widths(animation);
    if (old_closed != animation->closed_width || old_full != animation->full_width)
        set_target(animation, animation->tick != 0);
}

static gboolean stop_scroll(GtkWidget *widget, GdkEventScroll *event, gpointer data) {
    (void)widget; (void)event; (void)data;
    return TRUE;
}

static void keep_left(GtkAdjustment *adjustment, gpointer data) {
    (void)data;
    if (gtk_adjustment_get_value(adjustment) != 0)
        gtk_adjustment_set_value(adjustment, 0);
}

static GtkWidget *find_workspaces(GtkWidget *widget) {
    if (GTK_IS_BOX(widget) && g_strcmp0(gtk_widget_get_name(widget), "workspaces") == 0)
        return widget;
    if (!GTK_IS_CONTAINER(widget)) return NULL;
    GList *children = gtk_container_get_children(GTK_CONTAINER(widget));
    GtkWidget *found = NULL;
    for (GList *it = children; it && !found; it = it->next)
        found = find_workspaces(GTK_WIDGET(it->data));
    g_list_free(children);
    return found;
}

static void apply_state(Animation *animation) {
    JsonParser *parser = json_parser_new();
    if (!json_parser_load_from_file(parser, animation->state_path, NULL)) {
        g_object_unref(parser);
        return;
    }
    JsonNode *node = json_parser_get_root(parser);
    if (node && JSON_NODE_HOLDS_OBJECT(node)) {
        JsonObject *state = json_node_get_object(node);
        gboolean expanded = FALSE;
        if (json_object_has_member(state, "expanded")) {
            expanded = json_object_get_boolean_member(state, "expanded");
        } else if (json_object_has_member(state, "mode") &&
                   json_object_has_member(state, "active") &&
                   json_object_has_member(state, "occupied")) {
            /* Compatible with the controller until the service is restarted. */
            const char *mode = json_object_get_string_member(state, "mode");
            expanded = g_strcmp0(mode, "pinned") == 0 ||
                (g_strcmp0(mode, "auto") == 0 &&
                 (json_object_get_int_member(state, "active") >= 7 ||
                  json_object_get_boolean_member(state, "occupied")));
        }
        gboolean changed = !animation->state_known || animation->expanded != expanded;
        gboolean animate = animation->state_known;
        animation->state_known = TRUE;
        animation->expanded = expanded;
        if (changed && animation->viewport) {
            measure_widths(animation);
            set_target(animation, animate);
        }
    }
    g_object_unref(parser);
}

static void state_changed(GFileMonitor *monitor, GFile *file, GFile *other,
                          GFileMonitorEvent event, gpointer data) {
    (void)monitor; (void)event;
    char *name = g_file_get_basename(file);
    char *destination = other ? g_file_get_basename(other) : NULL;
    /* Atomic replacement arrives as state.json.tmp -> state.json. */
    if (g_strcmp0(name, "state.json") == 0 || g_strcmp0(destination, "state.json") == 0)
        apply_state(data);
    g_free(destination);
    g_free(name);
}

static gboolean attach_workspace(gpointer data) {
    Animation *animation = data;
    GtkWidget *window = gtk_widget_get_toplevel(animation->root);
    if (!GTK_IS_WINDOW(window)) return G_SOURCE_CONTINUE;
    animation->workspaces = find_workspaces(window);
    if (!animation->workspaces) return G_SOURCE_CONTINUE;
    GtkWidget *parent = gtk_widget_get_parent(animation->workspaces);
    if (!GTK_IS_BIN(parent)) return G_SOURCE_CONTINUE;
    g_object_add_weak_pointer(G_OBJECT(animation->workspaces), (gpointer *)&animation->workspaces);
    measure_widths(animation);
    /* The native box keeps all its original buttons as direct children, so
     * Waybar can still sort, add, remove and update them without interference. */
    animation->viewport = gtk_scrolled_window_new(NULL, NULL);
    gtk_widget_set_name(animation->viewport, "workspace-viewport");
    gtk_scrolled_window_set_policy(GTK_SCROLLED_WINDOW(animation->viewport), GTK_POLICY_EXTERNAL, GTK_POLICY_NEVER);
    gtk_scrolled_window_set_shadow_type(GTK_SCROLLED_WINDOW(animation->viewport), GTK_SHADOW_NONE);
    gtk_scrolled_window_set_kinetic_scrolling(GTK_SCROLLED_WINDOW(animation->viewport), FALSE);
    gtk_scrolled_window_set_propagate_natural_width(GTK_SCROLLED_WINDOW(animation->viewport), FALSE);
    gtk_scrolled_window_set_propagate_natural_height(GTK_SCROLLED_WINDOW(animation->viewport), TRUE);
    g_object_ref(animation->workspaces);
    gtk_container_remove(GTK_CONTAINER(parent), animation->workspaces);
    gtk_container_add(GTK_CONTAINER(animation->viewport), animation->workspaces);
    g_object_unref(animation->workspaces);
    GtkWidget *inner = gtk_bin_get_child(GTK_BIN(animation->viewport));
    if (GTK_IS_VIEWPORT(inner)) gtk_viewport_set_shadow_type(GTK_VIEWPORT(inner), GTK_SHADOW_NONE);
    gtk_container_add(GTK_CONTAINER(parent), animation->viewport);
    g_object_add_weak_pointer(G_OBJECT(animation->viewport), (gpointer *)&animation->viewport);
    g_signal_connect(animation->workspaces, "size-allocate", G_CALLBACK(workspaces_allocated), animation);
    g_signal_connect_after(animation->viewport, "size-allocate", G_CALLBACK(viewport_allocated), animation);
    g_signal_connect(animation->viewport, "scroll-event", G_CALLBACK(stop_scroll), NULL);
    GtkAdjustment *adjustment = gtk_scrolled_window_get_hadjustment(GTK_SCROLLED_WINDOW(animation->viewport));
    g_signal_connect(adjustment, "value-changed", G_CALLBACK(keep_left), NULL);
    apply_state(animation);
    set_target(animation, FALSE);
    gtk_widget_show_all(animation->viewport);
    report_layout(animation, "attached");
    animation->attach_source = 0;
    return G_SOURCE_REMOVE;
}

void *wbcffi_init(const wbcffi_init_info *info,
                 const wbcffi_config_entry *entries, size_t entries_len) {
    (void)entries; (void)entries_len;
    Animation *animation = g_new0(Animation, 1);
    animation->root = GTK_WIDGET(info->get_root_widget(info->obj));
    gtk_widget_set_name(animation->root, "workspace-animation");
    const char *signature = g_getenv("HYPRLAND_INSTANCE_SIGNATURE");
    char *hash = g_compute_checksum_for_string(G_CHECKSUM_SHA256,
                                               signature ? signature : "no-session", -1);
    hash[12] = '\0';
    char *name = g_strconcat("waybar-workspaces-", hash, NULL);
    char *directory = g_build_filename(g_get_user_runtime_dir(), name, NULL);
    g_mkdir_with_parents(directory, 0700);
    animation->state_path = g_build_filename(directory, "state.json", NULL);
    GFile *folder = g_file_new_for_path(directory);
    animation->monitor = g_file_monitor_directory(folder, G_FILE_MONITOR_WATCH_MOVES, NULL, NULL);
    if (animation->monitor) {
        g_file_monitor_set_rate_limit(animation->monitor, 0);
        g_signal_connect(animation->monitor, "changed", G_CALLBACK(state_changed), animation);
    }
    g_object_unref(folder);
    g_free(directory); g_free(name); g_free(hash);
    animation->attach_source = g_timeout_add(16, attach_workspace, animation);
    report_layout(animation, "waiting-for-native-module");
    return animation;
}

void wbcffi_deinit(void *instance) {
    Animation *animation = instance;
    if (!animation) return;
    if (animation->attach_source) g_source_remove(animation->attach_source);
    if (animation->viewport) {
        g_signal_handlers_disconnect_by_data(animation->viewport, animation);
        if (animation->tick) gtk_widget_remove_tick_callback(animation->viewport, animation->tick);
        g_object_remove_weak_pointer(G_OBJECT(animation->viewport), (gpointer *)&animation->viewport);
    }
    if (animation->workspaces) {
        GList *buttons = gtk_container_get_children(GTK_CONTAINER(animation->workspaces));
        for (GList *it = buttons; it; it = it->next)
            g_signal_handlers_disconnect_by_data(it->data, animation);
        g_list_free(buttons);
        g_signal_handlers_disconnect_by_data(animation->workspaces, animation);
        g_object_remove_weak_pointer(G_OBJECT(animation->workspaces), (gpointer *)&animation->workspaces);
    }
    if (animation->monitor) {
        g_signal_handlers_disconnect_by_data(animation->monitor, animation);
        g_file_monitor_cancel(animation->monitor);
        g_object_unref(animation->monitor);
    }
    g_free(animation->state_path);
    g_free(animation);
}
