use super::{
    displays::{self, Mode, Monitor, Rule, Trial},
    store,
    widgets::{self, Settings},
};
use adw::prelude::*;
use gtk::{gio, glib};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};

fn page(title: &str, description: &str) -> (gtk::Widget, gtk::Box) {
    let body = gtk::Box::new(gtk::Orientation::Vertical, 20);
    body.set_margin_top(24);
    body.set_margin_bottom(24);
    body.set_margin_start(24);
    body.set_margin_end(24);
    let heading = gtk::Label::new(Some(title));
    heading.add_css_class("title-1");
    heading.set_xalign(0.);
    body.append(&heading);
    let note = status(description);
    body.append(&note);
    let clamp = adw::Clamp::builder().maximum_size(920).child(&body).build();
    let scroll = gtk::ScrolledWindow::builder()
        .child(&clamp)
        .hexpand(true)
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build();
    (scroll.upcast(), body)
}
fn status(text: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(text));
    l.set_wrap(true);
    l.set_xalign(0.);
    l.set_selectable(true);
    l.add_css_class("dim-label");
    l
}
fn group(title: &str) -> adw::PreferencesGroup {
    adw::PreferencesGroup::builder().title(title).build()
}
fn row(group: &adw::PreferencesGroup, title: &str, control: &impl IsA<gtk::Widget>) {
    let row = adw::ActionRow::builder().title(title).build();
    control.set_valign(gtk::Align::Center);
    row.add_suffix(control);
    group.add(&row);
}
fn spin(value: f64, min: f64, max: f64, step: f64, digits: u32) -> gtk::SpinButton {
    let spin = gtk::SpinButton::with_range(min, max, step);
    spin.set_digits(digits);
    spin.set_value(value);
    spin.set_numeric(true);
    spin
}
fn transparency(group: &adw::PreferencesGroup, title: &str, alpha: f64) -> gtk::Scale {
    let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0., 100., 1.);
    scale.set_size_request(220, -1);
    scale.set_draw_value(true);
    scale.set_digits(0);
    scale.set_value((1. - alpha) * 100.);
    scale.set_tooltip_text(Some("0 %: fondo opaco · 100 %: fondo transparente"));
    row(group, title, &scale);
    scale
}

pub fn widgets_page(window: &adw::ApplicationWindow) -> gtk::Widget {
    let (root, body) = page(
        "Barra y widgets",
        "Cada porcentaje controla la transparencia del fondo. Los textos e iconos conservan su opacidad. Los cambios se guardan al pulsar Aplicar.",
    );
    let baseline = match store::read_optional(&Settings::path()) {
        Ok(s) => s,
        Err(e) => {
            body.append(&status(&format!("{e:#}")));
            return root;
        }
    };
    let settings = match Settings::parse(baseline.as_deref()) {
        Ok(s) => s,
        Err(e) => {
            body.append(&status(&format!("{e:#}")));
            return root;
        }
    };
    let bars = group("Transparencia de la barra");
    let bar = transparency(&bars, "Waybar", settings.bar_opacity);
    body.append(&bars);
    let panel_group = group("Desplegables de la derecha");
    let panels: Vec<_> = widgets::PANELS
        .iter()
        .map(|(id, name)| (*id, transparency(&panel_group, name, settings.panel(id))))
        .collect();
    body.append(&panel_group);
    let resource = group("Monitor de recursos");
    let opacity = transparency(&resource, "Transparencia", settings.sysmon_opacity);
    let anchor = gtk::DropDown::from_strings(&[
        "Arriba · izquierda",
        "Arriba · centro",
        "Arriba · derecha",
        "Centro · izquierda",
        "Centro",
        "Centro · derecha",
        "Abajo · izquierda",
        "Abajo · centro",
        "Abajo · derecha",
    ]);
    anchor.set_selected(
        widgets::ANCHORS
            .iter()
            .position(|a| *a == settings.anchor)
            .unwrap_or(2) as u32,
    );
    row(&resource, "Anclaje", &anchor);
    let x = spin(settings.x as f64, -16384., 16384., 1., 0);
    row(&resource, "Desplazamiento horizontal (px)", &x);
    let y = spin(settings.y as f64, -16384., 16384., 1., 0);
    row(&resource, "Desplazamiento vertical (px)", &y);
    body.append(&resource);
    let sections_group = group("Contenido del monitor de recursos");
    let sections: Vec<_> = widgets::SECTIONS
        .iter()
        .map(|(id, name)| {
            let toggle = gtk::Switch::new();
            toggle.set_active(settings.visible(id));
            row(&sections_group, name, &toggle);
            (*id, toggle)
        })
        .collect();
    body.append(&sections_group);
    body.append(&status("La posición se aplica a las instancias del monitor de recursos que estén abiertas. Las temperaturas sin sensor disponible se muestran como —."));
    let apply = gtk::Button::with_label("Aplicar barra y widgets");
    apply.add_css_class("suggested-action");
    apply.set_halign(gtk::Align::Start);
    let reload = gtk::Button::with_label("Recargar valores guardados");
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    actions.append(&apply);
    actions.append(&reload);
    body.append(&actions);
    let note = status("Sin cambios aplicados");
    body.append(&note);
    let baseline = Rc::new(RefCell::new(baseline));
    let controls = (
        bar.clone(),
        opacity.clone(),
        anchor.clone(),
        x.clone(),
        y.clone(),
        panels.clone(),
        sections.clone(),
    );
    let baseline_reload = baseline.clone();
    let note_reload = note.clone();
    reload.connect_clicked(move |_| {
        let result = store::read_optional(&Settings::path())
            .and_then(|text| Settings::parse(text.as_deref()).map(|settings| (text, settings)));
        match result {
            Ok((text, settings)) => {
                let (bar, opacity, anchor, x, y, panels, sections) = &controls;
                bar.set_value((1. - settings.bar_opacity) * 100.);
                opacity.set_value((1. - settings.sysmon_opacity) * 100.);
                anchor.set_selected(
                    widgets::ANCHORS
                        .iter()
                        .position(|a| *a == settings.anchor)
                        .unwrap_or(2) as u32,
                );
                x.set_value(settings.x as f64);
                y.set_value(settings.y as f64);
                for (id, scale) in panels {
                    scale.set_value((1. - settings.panel(id)) * 100.);
                }
                for (id, toggle) in sections {
                    toggle.set_active(settings.visible(id));
                }
                *baseline_reload.borrow_mut() = text;
                note_reload.set_text("Valores guardados recargados.");
            }
            Err(e) => note_reload.set_text(&format!("{e:#}")),
        }
    });
    let busy = Rc::new(Cell::new(false));
    let busy_close = busy.clone();
    window.connect_close_request(move |_| {
        if busy_close.get() {
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    let body_weak = body.downgrade();
    apply.connect_clicked(move |_| {
        let mut settings = settings.clone();
        settings.bar_opacity = 1. - bar.value() / 100.;
        settings.sysmon_opacity = 1. - opacity.value() / 100.;
        settings.anchor = widgets::ANCHORS[anchor.selected() as usize].into();
        settings.x = x.value_as_int();
        settings.y = y.value_as_int();
        for (id, scale) in &panels {
            settings
                .panel_opacity
                .insert((*id).into(), 1. - scale.value() / 100.);
        }
        for (id, toggle) in &sections {
            settings.sections.insert((*id).into(), toggle.is_active());
        }
        let baseline_text = baseline.borrow().clone();
        let baseline = baseline.clone();
        let note = note.clone();
        let Some(body) = body_weak.upgrade() else {
            return;
        };
        body.set_sensitive(false);
        busy.set(true);
        let busy = busy.clone();
        note.set_text("Guardando y actualizando barra y widgets…");
        glib::spawn_future_local(async move {
            let saved = serde_json::to_string_pretty(&settings).unwrap();
            let result =
                gio::spawn_blocking(move || widgets::apply(&settings, baseline_text)).await;
            body.set_sensitive(true);
            busy.set(false);
            match result {
                Ok(Ok(messages)) => {
                    *baseline.borrow_mut() = Some(saved);
                    note.set_text(&messages.join("\n"));
                }
                Ok(Err(e)) => note.set_text(&format!("{e:#}")),
                Err(_) => note.set_text("Falló la tarea de guardado"),
            }
        });
    });
    root
}

struct OutputRow {
    monitor: Monitor,
    enabled: gtk::Switch,
    resolution: gtk::DropDown,
    resolutions: Vec<(u32, u32)>,
    refresh: gtk::DropDown,
    rates: Rc<RefCell<Vec<Mode>>>,
    x: gtk::SpinButton,
    y: gtk::SpinButton,
    scale: gtk::SpinButton,
}
impl OutputRow {
    fn build(monitor: Monitor, body: &gtk::Box) -> Self {
        let group = group(&format!("{} · {}", monitor.name, monitor.description));
        let enabled = gtk::Switch::new();
        enabled.set_active(!monitor.disabled);
        row(&group, "Pantalla encendida", &enabled);
        let mut modes = monitor.modes();
        if !monitor.disabled
            && !modes.iter().any(|m| {
                m.width == monitor.width
                    && m.height == monitor.height
                    && (m.hz - monitor.refresh_rate).abs() < 0.001
            })
            && let Some(mode) = Mode::parse(&monitor.snapshot().mode)
        {
            modes.push(mode);
        }
        let mut resolutions = Vec::new();
        for m in &modes {
            if !resolutions.contains(&(m.width, m.height)) {
                resolutions.push((m.width, m.height));
            }
        }
        let names: Vec<_> = resolutions
            .iter()
            .map(|(w, h)| format!("{w} × {h}"))
            .collect();
        let resolution =
            gtk::DropDown::from_strings(&names.iter().map(String::as_str).collect::<Vec<_>>());
        let selected = resolutions
            .iter()
            .position(|s| *s == (monitor.width, monitor.height))
            .unwrap_or(0);
        resolution.set_selected(selected as u32);
        row(&group, "Resolución", &resolution);
        let refresh = gtk::DropDown::from_strings(&[]);
        row(&group, "Frecuencia (Hz)", &refresh);
        let rates = Rc::new(RefCell::new(Vec::new()));
        let update = {
            let resolutions = resolutions.clone();
            let refresh = refresh.clone();
            let rates = rates.clone();
            let hz = monitor.refresh_rate;
            move |selected: u32| {
                let available: Vec<_> = modes
                    .iter()
                    .filter(|m| resolutions.get(selected as usize) == Some(&(m.width, m.height)))
                    .cloned()
                    .collect();
                let labels: Vec<_> = available
                    .iter()
                    .map(|m| format!("{:.3} Hz", m.hz))
                    .collect();
                refresh.set_model(Some(&gtk::StringList::new(
                    &labels.iter().map(String::as_str).collect::<Vec<_>>(),
                )));
                refresh.set_selected(
                    available
                        .iter()
                        .position(|m| (m.hz - hz).abs() < 0.01)
                        .unwrap_or(0) as u32,
                );
                *rates.borrow_mut() = available;
            }
        };
        update(selected as u32);
        resolution.connect_selected_notify(move |drop| update(drop.selected()));
        let x = spin(monitor.x as f64, -65536., 65536., 1., 0);
        row(&group, "Posición X (px lógicos)", &x);
        let y = spin(monitor.y as f64, -65536., 65536., 1., 0);
        row(&group, "Posición Y (px lógicos)", &y);
        let scale = spin(monitor.scale, 0.25, 8., 0.25, 2);
        row(&group, "Escala", &scale);
        if resolutions.is_empty() {
            group.set_description(Some(
                "La pantalla no anuncia modos. Vuelve a detectar tras encenderla.",
            ));
            resolution.set_sensitive(false);
            refresh.set_sensitive(false);
        }
        if !monitor.mirror_of.is_empty() && monitor.mirror_of != "none" {
            group.set_description(Some(&format!(
                "Espejo de {}. Se conserva el espejo y la rotación actuales.",
                monitor.mirror_of
            )));
        }
        body.append(&group);
        Self {
            monitor,
            enabled,
            resolution,
            resolutions,
            refresh,
            rates,
            x,
            y,
            scale,
        }
    }
    fn rule(&self) -> anyhow::Result<Rule> {
        let mut rule = self.monitor.snapshot();
        rule.enabled = self.enabled.is_active();
        if rule.enabled {
            let rates = self.rates.borrow();
            let mode = rates
                .get(self.refresh.selected() as usize)
                .ok_or_else(|| anyhow::anyhow!("{} no anuncia modos disponibles", rule.name))?;
            if self.resolutions.get(self.resolution.selected() as usize)
                != Some(&(mode.width, mode.height))
            {
                anyhow::bail!("Selecciona una resolución válida");
            }
            rule.mode = mode.raw.clone();
        }
        rule.x = self.x.value_as_int();
        rule.y = self.y.value_as_int();
        rule.scale = self.scale.value();
        Ok(rule)
    }
}
struct DisplaysPage {
    body: gtk::Box,
    list: gtk::Box,
    note: gtk::Label,
    controls: gtk::Box,
    outputs: RefCell<Vec<OutputRow>>,
    busy: Cell<bool>,
    trial: RefCell<Option<Trial>>,
    notice: RefCell<Option<String>>,
    window: glib::WeakRef<adw::ApplicationWindow>,
}
impl DisplaysPage {
    fn refresh(self: &Rc<Self>) {
        if self.busy.replace(true) {
            return;
        }
        self.controls.set_sensitive(false);
        self.list.set_sensitive(false);
        self.note
            .set_text("Detectando pantallas y modos disponibles…");
        let this = self.clone();
        glib::spawn_future_local(async move {
            let result = gio::spawn_blocking(displays::monitors).await;
            this.busy.set(false);
            this.controls.set_sensitive(true);
            this.list.set_sensitive(true);
            match result {
                Ok(Ok(monitors))=>{
                    while let Some(child)=this.list.first_child() {this.list.remove(&child);}
                    *this.outputs.borrow_mut()=monitors.into_iter().map(|m|OutputRow::build(m,&this.list)).collect();
                    this.note.set_text(this.notice.borrow_mut().take().as_deref().unwrap_or("Selecciona una pantalla, resolución y frecuencia. Prueba los cambios antes de guardarlos."));
                },
                Ok(Err(e))=>this.note.set_text(&format!("{e:#}\nAbre esta app dentro de tu sesión de Hyprland y pulsa Volver a detectar.")),
                Err(_)=>this.note.set_text("Falló la consulta de pantallas"),
            }
        });
    }
    fn test(self: &Rc<Self>) {
        if self.busy.get() {
            return;
        }
        let rules: anyhow::Result<Vec<_>> =
            self.outputs.borrow().iter().map(OutputRow::rule).collect();
        let rules = match rules {
            Ok(r) => r,
            Err(e) => {
                self.note.set_text(&format!("{e:#}"));
                return;
            }
        };
        self.busy.set(true);
        self.body.set_sensitive(false);
        self.note.set_text("Probando la nueva configuración…");
        let this = self.clone();
        glib::spawn_future_local(async move {
            match gio::spawn_blocking(move || Trial::begin(rules)).await {
                Ok(Ok(trial)) => {
                    *this.trial.borrow_mut() = Some(trial);
                    this.confirm_dialog();
                }
                Ok(Err(e)) => {
                    this.note.set_text(&format!("{e:#}"));
                    this.busy.set(false);
                    this.body.set_sensitive(true);
                }
                Err(_) => {
                    this.note.set_text(
                        "Falló la prueba. El proceso de recuperación restaurará las pantallas.",
                    );
                    this.busy.set(false);
                    this.body.set_sensitive(true);
                }
            }
        });
    }
    fn confirm_dialog(self: &Rc<Self>) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let seconds = self
            .trial
            .borrow()
            .as_ref()
            .map(Trial::confirmation_seconds)
            .unwrap_or(0);
        let dialog = adw::MessageDialog::new(
            Some(&window),
            Some("¿Conservar estos ajustes de pantalla?"),
            Some(&format!(
                "Se restaurará la configuración anterior en {seconds} segundos."
            )),
        );
        dialog.add_responses(&[("revert", "Restaurar"), ("keep", "Conservar")]);
        dialog.set_default_response(Some("revert"));
        dialog.set_close_response("revert");
        dialog.set_response_appearance("keep", adw::ResponseAppearance::Suggested);
        let finished = Rc::new(Cell::new(false));
        let done = finished.clone();
        let weak = dialog.downgrade();
        let start = Instant::now();
        glib::timeout_add_local(Duration::from_millis(200), move || {
            if done.get() {
                return glib::ControlFlow::Break;
            }
            let Some(dialog) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            let remaining = seconds.saturating_sub(start.elapsed().as_secs());
            dialog.set_body(&format!(
                "Se restaurará la configuración anterior en {remaining} segundos."
            ));
            if remaining == 0 {
                dialog.response("revert");
                return glib::ControlFlow::Break;
            }
            glib::ControlFlow::Continue
        });
        let this = self.clone();
        dialog.connect_response(None, move |_, response| {
            if finished.replace(true) {
                return;
            }
            let Some(mut trial) = this.trial.borrow_mut().take() else {
                return;
            };
            let keep = response == "keep";
            let this = this.clone();
            glib::spawn_future_local(async move {
                let result = gio::spawn_blocking(move || {
                    if keep {
                        trial
                            .confirm()
                            .map(|p| format!("Pantallas guardadas. Respaldo: {}", p.display()))
                    } else {
                        trial
                            .revert()
                            .map(|_| "Configuración anterior restaurada.".into())
                    }
                })
                .await;
                this.busy.set(false);
                this.body.set_sensitive(true);
                let message = match result {
                    Ok(Ok(message)) => message,
                    Ok(Err(e)) => format!("{e:#}"),
                    Err(_) => "Falló la operación de pantallas; revisa la configuración".into(),
                };
                *this.notice.borrow_mut() = Some(message);
                this.refresh();
            });
        });
        dialog.present();
    }
}
pub fn displays_page(window: &adw::ApplicationWindow) -> gtk::Widget {
    let (root, body) = page(
        "Pantallas",
        "Configura las pantallas conectadas, su posición, escala, resolución y frecuencia. Solo se ofrecen los modos anunciados por cada pantalla. Conserva la rotación y los espejos existentes.",
    );
    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let refresh = gtk::Button::with_label("Volver a detectar");
    controls.append(&refresh);
    let apply = gtk::Button::with_label("Probar cambios");
    apply.add_css_class("suggested-action");
    controls.append(&apply);
    body.append(&controls);
    let note = status("");
    body.append(&note);
    let list = gtk::Box::new(gtk::Orientation::Vertical, 20);
    body.append(&list);
    let this = Rc::new(DisplaysPage {
        body,
        list,
        note,
        controls,
        outputs: RefCell::default(),
        busy: Cell::new(false),
        trial: RefCell::default(),
        notice: RefCell::default(),
        window: window.downgrade(),
    });
    let state = this.clone();
    refresh.connect_clicked(move |_| state.refresh());
    let state = this.clone();
    apply.connect_clicked(move |_| state.test());
    let weak = Rc::downgrade(&this);
    window.connect_close_request(move |_| {
        if weak.upgrade().is_some_and(|s| s.busy.get()) {
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    this.refresh();
    root
}
