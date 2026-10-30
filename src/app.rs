use std::cell::RefCell;
use std::rc::Rc;

use gloo_timers::callback::Timeout;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlDialogElement, HtmlInputElement, HtmlSelectElement, KeyboardEvent};
use yew::prelude::*;

use crate::api;
use crate::components::{icon, StatePanel};
use crate::format;
use crate::models::{Quota, Settings};
use crate::storage;
use crate::views::{CompareView, KeywordsView, PanelView, PlannerView, TimesView, TitlesView, TrendsView};

const REGIONS: [(&str, &str); 13] = [
    ("ES", "España"),
    ("MX", "México"),
    ("AR", "Argentina"),
    ("CO", "Colombia"),
    ("CL", "Chile"),
    ("PE", "Perú"),
    ("US", "Estados Unidos"),
    ("GB", "Reino Unido"),
    ("DE", "Alemania"),
    ("FR", "Francia"),
    ("IT", "Italia"),
    ("BR", "Brasil"),
    ("PT", "Portugal"),
];

const REFRESH_OPTIONS: [(u32, &str); 5] = [
    (0, "Solo manual"),
    (60, "Cada minuto"),
    (300, "Cada 5 minutos"),
    (900, "Cada 15 minutos"),
    (1800, "Cada 30 minutos"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Panel,
    Trends,
    Keywords,
    Compare,
    Times,
    Titles,
    Planner,
}

impl View {
    pub fn hash(self) -> &'static str {
        match self {
            View::Panel => "#panel",
            View::Trends => "#tendencias",
            View::Keywords => "#keywords",
            View::Compare => "#comparador",
            View::Times => "#horarios",
            View::Titles => "#titulos",
            View::Planner => "#planificador",
        }
    }

    fn label(self) -> &'static str {
        match self {
            View::Panel => "Panel",
            View::Trends => "Tendencias",
            View::Keywords => "Keywords",
            View::Compare => "Comparador",
            View::Times => "Horarios",
            View::Titles => "Títulos",
            View::Planner => "Planificador",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            View::Panel => "Panel del canal",
            View::Trends => "Radar de tendencias",
            View::Keywords => "Investigación de keywords",
            View::Compare => "Comparador de canales",
            View::Times => "Horarios reales",
            View::Titles => "Analizador de títulos",
            View::Planner => "Planificador",
        }
    }

    pub fn from_hash(hash: &str) -> Self {
        match hash.trim_start_matches('#') {
            "tendencias" => View::Trends,
            "keywords" => View::Keywords,
            "comparador" => View::Compare,
            "horarios" => View::Times,
            "titulos" => View::Titles,
            "planificador" => View::Planner,
            _ => View::Panel,
        }
    }

    const ALL: [View; 7] = [
        View::Panel,
        View::Trends,
        View::Keywords,
        View::Compare,
        View::Times,
        View::Titles,
        View::Planner,
    ];
}

#[derive(Clone, Debug, PartialEq)]
pub enum KeyStatus {
    Unknown,
    Checking,
    Valid,
    Invalid(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfirmKind {
    DisconnectChannel,
    DisconnectKey,
    ClearData,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConfirmRequest {
    pub kind: ConfirmKind,
    pub title: String,
    pub body: String,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ToastMsg {
    pub id: u32,
    pub text: String,
}

#[derive(Clone, PartialEq)]
pub struct AppState {
    pub settings: Settings,
    pub quota: Quota,
    pub status: KeyStatus,
    pub view: View,
    pub settings_open: bool,
    pub confirm: Option<ConfirmRequest>,
    pub toast: Option<ToastMsg>,
    pub toast_seq: u32,
}

fn current_hash() -> String {
    web_sys::window()
        .and_then(|window| window.location().hash().ok())
        .unwrap_or_default()
}

impl AppState {
    fn boot() -> Self {
        let settings = Settings::load();
        let status = if settings.api_key.is_empty() {
            KeyStatus::Unknown
        } else {
            KeyStatus::Checking
        };
        AppState {
            settings,
            quota: Quota::load(),
            status,
            view: View::from_hash(&current_hash()),
            settings_open: false,
            confirm: None,
            toast: None,
            toast_seq: 0,
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct AppCtx {
    pub state: UseStateHandle<AppState>,
    current: Rc<RefCell<AppState>>,
}

impl AppCtx {
    // UseStateHandle clones keep the value from their render, so mutations and
    // follow-up reads in the same tick go through `current` instead.
    fn update(&self, mutate: impl FnOnce(&mut AppState)) {
        let next = {
            let mut state = self.current.borrow_mut();
            mutate(&mut state);
            state.clone()
        };
        self.state.set(next);
    }

    fn snapshot(&self) -> AppState {
        self.current.borrow().clone()
    }

    pub fn open_settings(&self) {
        self.update(|state| state.settings_open = true);
    }

    pub fn close_settings(&self) {
        self.update(|state| state.settings_open = false);
    }

    pub fn toast(&self, text: impl Into<String>) {
        self.update(|state| {
            let id = state.toast_seq.wrapping_add(1);
            state.toast_seq = id;
            state.toast = Some(ToastMsg {
                id,
                text: text.into(),
            });
        });
    }

    pub fn add_quota(&self, units: u32) {
        let today = format::local_date_key(format::now_ms());
        let mut quota = self.snapshot().quota;
        if quota.date != today {
            quota = Quota {
                date: today,
                units: 0,
                calls: 0,
            };
        }
        quota.units = quota.units.saturating_add(units);
        quota.calls += 1;
        quota.save();
        self.update(|state| state.quota = quota);
    }

    pub fn reset_quota(&self) {
        storage::remove("quota");
        self.update(|state| state.quota = Quota::load());
    }

    pub fn set_status(&self, status: KeyStatus) {
        self.update(|state| state.status = status);
    }

    pub fn save_settings(&self, settings: Settings) {
        let key_changed = settings.api_key.trim() != self.snapshot().settings.api_key.trim();
        let mut settings = settings;
        settings.api_key = settings.api_key.trim().to_string();
        settings.channel_ref = settings.channel_ref.trim().to_string();
        settings.save();
        let has_key = !settings.api_key.is_empty();
        let key = settings.api_key.clone();
        self.update(|state| {
            state.settings = settings;
            state.status = if has_key {
                KeyStatus::Checking
            } else {
                KeyStatus::Unknown
            };
        });
        if has_key && key_changed {
            self.check_key_with(key, false);
        }
    }

    pub fn set_channel_ref(&self, reference: String) {
        let mut settings = self.snapshot().settings;
        settings.channel_ref = reference;
        settings.save();
        self.update(|state| state.settings = settings);
    }

    pub fn check_key(&self, notify: bool) {
        let key = self.snapshot().settings.api_key;
        self.check_key_with(key, notify);
    }

    fn check_key_with(&self, key: String, notify: bool) {
        if key.is_empty() {
            self.set_status(KeyStatus::Unknown);
            return;
        }
        self.set_status(KeyStatus::Checking);
        let ctx = self.clone();
        spawn_local(async move {
            match api::test_key(&key).await {
                Ok(()) => ctx.set_status(KeyStatus::Valid),
                Err(error) => {
                    ctx.set_status(KeyStatus::Invalid(error.message.clone()));
                    if notify {
                        ctx.toast(error.message);
                    }
                }
            }
        });
    }

    pub fn api_failed(&self, error: &api::ApiError) {
        if error.kind == api::ApiErrorKind::InvalidKey {
            self.set_status(KeyStatus::Invalid(error.message.clone()));
        }
    }

    pub fn open_confirm(&self, kind: ConfirmKind) {
        let (title, body, label) = match kind {
            ConfirmKind::DisconnectChannel => (
                "Desconectar el canal",
                "El panel dejará de consultar este canal. Las mediciones guardadas se conservan en este navegador.",
                "Desconectar",
            ),
            ConfirmKind::DisconnectKey => (
                "Desconectar la API key",
                "Se borrará la clave guardada en este navegador. Podrás volver a conectarla cuando quieras.",
                "Desconectar",
            ),
            ConfirmKind::ClearData => (
                "Borrar los datos guardados",
                "Se eliminarán el canal, las mediciones, las comparativas, las keywords guardadas y el planificador. La API key y la región se mantienen.",
                "Borrar datos",
            ),
        };
        self.update(|state| {
            state.confirm = Some(ConfirmRequest {
                kind,
                title: title.to_string(),
                body: body.to_string(),
                label: label.to_string(),
            });
        });
    }

    pub fn close_confirm(&self) {
        self.update(|state| state.confirm = None);
    }

    pub fn apply_confirm(&self, kind: ConfirmKind) {
        self.update(|state| state.confirm = None);
        match kind {
            ConfirmKind::DisconnectChannel => {
                let mut settings = self.snapshot().settings;
                settings.channel_ref.clear();
                settings.save();
                self.update(|state| state.settings = settings);
                self.toast("Canal desconectado");
            }
            ConfirmKind::DisconnectKey => {
                let mut settings = self.snapshot().settings;
                settings.api_key.clear();
                settings.save();
                self.update(|state| {
                    state.settings = settings;
                    state.status = KeyStatus::Unknown;
                });
                self.toast("Clave desconectada");
            }
            ConfirmKind::ClearData => {
                storage::clear_saved_data();
                if let Some(window) = web_sys::window() {
                    let _ = window.location().reload();
                }
            }
        }
    }
}

fn status_text(ctx: &AppCtx) -> &'static str {
    if ctx.state.settings.api_key.is_empty() {
        return "Conectar API";
    }
    match ctx.state.status {
        KeyStatus::Checking => "Comprobando…",
        KeyStatus::Valid => "API conectada",
        KeyStatus::Invalid(_) => "Revisar API",
        KeyStatus::Unknown => "API configurada",
    }
}

#[function_component(App)]
pub fn app() -> Html {
    let state = use_state(AppState::boot);
    let current = use_mut_ref(|| (*state).clone());
    let ctx = AppCtx {
        state: state.clone(),
        current: current.clone(),
    };

    // Hash routing.
    {
        let state = state.clone();
        let current = current.clone();
        use_effect_with((), move |_| {
            let Some(window) = web_sys::window() else {
                return Box::new(|| {}) as Box<dyn FnOnce()>;
            };
            let listener_window = window.clone();
            let listener_state = state.clone();
            let listener_current = current.clone();
            let listener = Closure::<dyn FnMut()>::new(move || {
                let view = View::from_hash(&current_hash());
                let mut next = listener_current.borrow().clone();
                if next.view != view {
                    next.view = view;
                    *listener_current.borrow_mut() = next.clone();
                    listener_state.set(next);
                }
                listener_window.scroll_to_with_x_and_y(0.0, 0.0);
            });
            let _ = window
                .add_event_listener_with_callback("hashchange", listener.as_ref().unchecked_ref());
            let cleanup_window = window.clone();
            Box::new(move || {
                let _ = cleanup_window
                    .remove_event_listener_with_callback("hashchange", listener.as_ref().unchecked_ref());
                drop(listener);
            }) as Box<dyn FnOnce()>
        });
    }

    // API status and quota reporting.
    {
        let ctx = ctx.clone();
        use_effect_with((), move |_| {
            let hook_ctx = ctx.clone();
            api::set_quota_hook(Some(Callback::from(move |units: u32| {
                hook_ctx.add_quota(units);
            })));
            if !ctx.snapshot().settings.api_key.is_empty() {
                ctx.check_key(false);
            }
            move || api::set_quota_hook(None)
        });
    }

    // Document title.
    {
        let view = state.view;
        use_effect_with(view, move |view| {
            if let Some(document) = web_sys::window().and_then(|window| window.document()) {
                let _ = document.set_title(&format!("{} · Pausiar Tools", view.title()));
            }
        });
    }

    // Toast auto-dismiss.
    {
        let toast_id = state.toast.as_ref().map(|toast| toast.id);
        let state = state.clone();
        let current = current.clone();
        use_effect_with(toast_id, move |id| {
            let timeout = id.map(|id| {
                Timeout::new(4500, move || {
                    let mut next = current.borrow().clone();
                    let still_same = next.toast.as_ref().map(|toast| toast.id) == Some(id);
                    if still_same {
                        next.toast = None;
                        *current.borrow_mut() = next.clone();
                        state.set(next);
                    }
                })
            });
            move || {
                if let Some(timeout) = timeout {
                    timeout.cancel();
                }
            }
        });
    }

    let toast = state.toast.as_ref().map(|toast| {
        html! {
            <div class="toast is-visible" role="status">
                <span class="toast__text">{ &toast.text }</span>
            </div>
        }
    });

    html! {
        <ContextProvider<AppCtx> context={ctx.clone()}>
            <a class="skip-link" href="#main">{ "Saltar al contenido" }</a>

            <header class="topbar">
                <div class="shell topbar__inner">
                    <a class="brand" href="#panel" aria-label="Pausiar Tools, ir al panel">
                        <span class="brand__mark" aria-hidden="true"><span></span><span></span></span>
                        <span class="brand__name">{ "Pausiar" }</span>
                        <span class="brand__tools">{ "tools" }</span>
                    </a>

                    <nav class="tabs" aria-label="Herramientas">
                        { for View::ALL.iter().map(|view| html! {
                            <a
                                href={view.hash()}
                                aria-current={if state.view == *view { "page" } else { "false" }}
                            >
                                { view.label() }
                            </a>
                        }) }
                    </nav>

                    <div class="topbar__actions">
                        <button
                            class={classes!("api-chip", matches!(state.status, KeyStatus::Valid).then_some("is-connected"))}
                            type="button"
                            onclick={{ let ctx = ctx.clone(); Callback::from(move |_: MouseEvent| ctx.open_settings()) }}
                            aria-haspopup="dialog"
                            title={match &state.status { KeyStatus::Invalid(message) => message.clone(), _ => String::new() }}
                        >
                            { icon("key", "") }
                            <span>{ status_text(&ctx) }</span>
                        </button>
                        <button
                            class="icon-btn"
                            type="button"
                            aria-label="Ajustes"
                            aria-haspopup="dialog"
                            onclick={{ let ctx = ctx.clone(); Callback::from(move |_: MouseEvent| ctx.open_settings()) }}
                        >
                            { icon("settings", "") }
                        </button>
                    </div>
                </div>
            </header>

            <main id="main" class="shell main">
                <section id="view-panel" class="view" data-view="panel" aria-labelledby="panel-title" hidden={state.view != View::Panel}>
                    <PanelView />
                </section>
                <section id="view-tendencias" class="view" data-view="tendencias" aria-labelledby="trends-title" hidden={state.view != View::Trends}>
                    <TrendsView />
                </section>
                <section id="view-keywords" class="view" data-view="keywords" aria-labelledby="keywords-title" hidden={state.view != View::Keywords}>
                    <KeywordsView />
                </section>
                <section id="view-comparador" class="view" data-view="comparador" aria-labelledby="compare-title" hidden={state.view != View::Compare}>
                    <CompareView />
                </section>
                <section id="view-horarios" class="view" data-view="horarios" aria-labelledby="times-title" hidden={state.view != View::Times}>
                    <TimesView />
                </section>
                <section id="view-titulos" class="view" data-view="titulos" aria-labelledby="analyzer-title" hidden={state.view != View::Titles}>
                    <TitlesView />
                </section>
                <section id="view-planificador" class="view" data-view="planificador" aria-labelledby="planner-title" hidden={state.view != View::Planner}>
                    <PlannerView />
                </section>
            </main>

            <footer class="footer">
                <div class="shell footer__inner">
                    <p class="footer__note">
                        { "Datos en vivo de YouTube Data API v3, sin backend. La key y tus análisis se guardan solo en este navegador. TikTok no publica estadísticas abiertas: aquí solo se analizan títulos y se planifica." }
                    </p>
                    <div class="footer__links">
                        <a href="https://developers.google.com/youtube/v3/determine_quota_cost" target="_blank" rel="noopener noreferrer">{ "Cuota de la API" }</a>
                        <a href="https://github.com/Pausiar/social-media-tools" target="_blank" rel="noopener noreferrer">{ "GitHub" }</a>
                        <button
                            class="link-btn"
                            type="button"
                            onclick={{ let ctx = ctx.clone(); Callback::from(move |_: MouseEvent| ctx.open_settings()) }}
                        >
                            { "Ajustes" }
                        </button>
                    </div>
                </div>
            </footer>

            <SettingsDialog />
            <ConfirmDialog />
            <div id="toasts" class="toast-stack" aria-live="polite">{ toast }</div>
        </ContextProvider<AppCtx>>
    }
}

#[function_component(SettingsDialog)]
fn settings_dialog() -> Html {
    let ctx = use_context::<AppCtx>().expect("app context");
    let dialog_ref = use_node_ref();
    let draft = use_state(|| ctx.state.settings.clone());
    let show_key = use_state(|| false);
    let open = ctx.state.settings_open;

    {
        let draft = draft.clone();
        let settings = ctx.state.settings.clone();
        use_effect_with(open, move |is_open| {
            if *is_open {
                draft.set(settings);
            }
        });
    }

    {
        let dialog_ref = dialog_ref.clone();
        use_effect_with(open, move |is_open| {
            if let Some(dialog) = dialog_ref.cast::<HtmlDialogElement>() {
                if *is_open && !dialog.open() {
                    let _ = dialog.show_modal();
                }
                if !*is_open && dialog.open() {
                    dialog.close();
                }
            }
        });
    }

    let on_key = {
        let draft = draft.clone();
        Callback::from(move |event: InputEvent| {
            let input = event.target_unchecked_into::<HtmlInputElement>();
            let mut next = (*draft).clone();
            next.api_key = input.value();
            draft.set(next);
        })
    };

    let on_region = {
        let draft = draft.clone();
        Callback::from(move |event: Event| {
            let select = event.target_unchecked_into::<HtmlSelectElement>();
            let mut next = (*draft).clone();
            next.region = select.value();
            draft.set(next);
        })
    };

    let on_refresh = {
        let draft = draft.clone();
        Callback::from(move |event: Event| {
            let select = event.target_unchecked_into::<HtmlSelectElement>();
            let mut next = (*draft).clone();
            next.refresh_secs = select.value().parse().unwrap_or(300);
            draft.set(next);
        })
    };

    let toggle_key = {
        let show_key = show_key.clone();
        Callback::from(move |_: MouseEvent| show_key.set(!*show_key))
    };

    let save = {
        let ctx = ctx.clone();
        let draft = draft.clone();
        Callback::from(move |_: MouseEvent| {
            ctx.save_settings((*draft).clone());
            ctx.close_settings();
            ctx.toast("Ajustes guardados");
        })
    };

    let test = {
        let ctx = ctx.clone();
        let draft = draft.clone();
        Callback::from(move |_: MouseEvent| {
            let settings = (*draft).clone();
            let has_key = !settings.api_key.trim().is_empty();
            let changed =
                ctx.snapshot().settings.api_key != settings.api_key.trim().to_string();
            ctx.save_settings(settings);
            if !has_key {
                ctx.toast("Añade una API key para probar la conexión");
            } else if !changed {
                ctx.check_key(true);
            }
        })
    };

    let close = {
        let ctx = ctx.clone();
        Callback::from(move |_: MouseEvent| ctx.close_settings())
    };

    let on_keydown = {
        let ctx = ctx.clone();
        Callback::from(move |event: KeyboardEvent| {
            if event.key() == "Escape" {
                event.prevent_default();
                ctx.close_settings();
            }
        })
    };

    let key_empty = ctx.state.settings.api_key.is_empty();
    let status = match &ctx.state.status {
        KeyStatus::Valid => html! { <p class="form-note">{ "Clave verificada: las consultas funcionan." }</p> },
        KeyStatus::Checking => html! { <p class="form-note">{ "Comprobando la clave con la API…" }</p> },
        KeyStatus::Invalid(message) => html! { <p class="form-note is-error">{ message }</p> },
        KeyStatus::Unknown => html! {
            <p class="form-note">{ "Sin clave no se pueden consultar datos reales de YouTube." }</p>
        },
    };

    html! {
        <dialog id="settings-dialog" class="dialog" ref={dialog_ref} onkeydown={on_keydown} aria-labelledby="settings-title">
            <div class="dialog__panel">
                <div class="dialog__head">
                    <div>
                        <h2 id="settings-title">{ "Ajustes" }</h2>
                        <p class="panel-sub">{ "La API key se guarda solo en este navegador y viaja directamente a Google." }</p>
                    </div>
                    <button class="icon-btn" type="button" aria-label="Cerrar ajustes" onclick={close.clone()}>
                        { icon("x", "") }
                    </button>
                </div>

                <div class="dialog__body">
                    <section class="settings-block">
                        <h3>{ "API key de YouTube" }</h3>
                        <div class="field">
                            <label for="api-key">{"Clave"}</label>
                            <div class="input-group">
                                <input
                                    id="api-key"
                                    class="input"
                                    type={if *show_key { "text" } else { "password" }}
                                    value={draft.api_key.clone()}
                                    oninput={on_key}
                                    placeholder="AIza…"
                                    autocomplete="off"
                                    spellcheck="false"
                                />
                                <button
                                    class={classes!("icon-btn", (*show_key).then_some("is-on"))}
                                    type="button"
                                    aria-label={if *show_key { "Ocultar clave" } else { "Mostrar clave" }}
                                    aria-pressed={show_key.to_string()}
                                    onclick={toggle_key}
                                >
                                    { icon("eye", "") }
                                </button>
                            </div>
                        </div>
                        { status }
                        <p class="hint">
                            { "Créala en Google Cloud, activa YouTube Data API v3 y restringe la clave a este dominio. " }
                            <a href="https://console.cloud.google.com/apis/credentials" target="_blank" rel="noopener noreferrer">{ "Abrir Google Cloud" }</a>
                        </p>
                        <div class="settings-actions">
                            <button class="btn btn--secondary" type="button" onclick={test}>
                                { icon("check", "") }
                                { "Guardar y probar" }
                            </button>
                            if !key_empty {
                                <button class="btn btn--danger" type="button" onclick={{
                                    let ctx = ctx.clone();
                                    Callback::from(move |_: MouseEvent| ctx.open_confirm(ConfirmKind::DisconnectKey))
                                }}>
                                    { "Desconectar clave" }
                                </button>
                            }
                        </div>
                    </section>

                    <section class="settings-block">
                        <h3>{ "Búsquedas" }</h3>
                        <div class="settings-grid">
                            <div class="field">
                                <label for="region">{"Región"}</label>
                                <select class="input" id="region" onchange={on_region}>
                                    { for REGIONS.iter().map(|(code, label)| html! {
                                        <option value={*code} selected={draft.region == *code}>{ *label }</option>
                                    }) }
                                </select>
                            </div>
                            <div class="field">
                                <label for="refresh">{"Actualizar el panel"}</label>
                                <select class="input" id="refresh" onchange={on_refresh}>
                                    { for REFRESH_OPTIONS.iter().map(|(secs, label)| html! {
                                        <option value={secs.to_string()} selected={draft.refresh_secs == *secs}>{ *label }</option>
                                    }) }
                                </select>
                            </div>
                        </div>
                    </section>

                    <section class="settings-block">
                        <div class="quota-head">
                            <h3>{ "Cuota estimada hoy" }</h3>
                            <span id="quota-label">{ format!("{} / {}", format::fmt_int(ctx.state.quota.units as u64), format::fmt_int(Quota::DAILY_LIMIT as u64)) }</span>
                        </div>
                        <progress class="quota-bar" value={ctx.state.quota.units.to_string()} max={Quota::DAILY_LIMIT.to_string()}></progress>
                        <p class="hint">
                            { format!("YouTube permite {} unidades diarias. Una búsqueda cuesta 100; el resto de consultas, 1. Este contador es una estimación local.", format::fmt_int(Quota::DAILY_LIMIT as u64)) }
                        </p>
                        <div class="settings-actions">
                            <button class="link-btn" type="button" onclick={{
                                let ctx = ctx.clone();
                                Callback::from(move |_: MouseEvent| { ctx.reset_quota(); ctx.toast("Contador reiniciado"); })
                            }}>
                                { "Reiniciar contador" }
                            </button>
                        </div>
                    </section>

                    <section class="settings-block">
                        <h3>{ "Datos locales" }</h3>
                        <p class="hint">{ "Se guardan en este navegador: canal, mediciones diarias, comparativas, keywords y planificador." }</p>
                        <div class="settings-actions">
                            <button class="btn btn--danger" type="button" onclick={{
                                let ctx = ctx.clone();
                                Callback::from(move |_: MouseEvent| ctx.open_confirm(ConfirmKind::ClearData))
                            }}>
                                { icon("trash", "") }
                                { "Borrar datos guardados" }
                            </button>
                        </div>
                    </section>
                </div>

                <div class="dialog__foot">
                    <button class="btn btn--quiet" type="button" onclick={close}>{ "Cerrar" }</button>
                    <button class="btn btn--primary" type="button" onclick={save}>{ "Guardar" }</button>
                </div>
            </div>
        </dialog>
    }
}

#[function_component(ConfirmDialog)]
fn confirm_dialog() -> Html {
    let ctx = use_context::<AppCtx>().expect("app context");
    let dialog_ref = use_node_ref();
    let request = ctx.state.confirm.clone();
    let open = request.is_some();

    {
        let dialog_ref = dialog_ref.clone();
        use_effect_with(open, move |is_open| {
            if let Some(dialog) = dialog_ref.cast::<HtmlDialogElement>() {
                if *is_open && !dialog.open() {
                    let _ = dialog.show_modal();
                }
                if !*is_open && dialog.open() {
                    dialog.close();
                }
            }
        });
    }

    let cancel = {
        let ctx = ctx.clone();
        Callback::from(move |_: MouseEvent| ctx.close_confirm())
    };
    let accept = {
        let ctx = ctx.clone();
        let request = request.clone();
        Callback::from(move |_: MouseEvent| {
            if let Some(request) = &request {
                ctx.apply_confirm(request.kind);
            }
        })
    };
    let on_keydown = {
        let ctx = ctx.clone();
        Callback::from(move |event: KeyboardEvent| {
            if event.key() == "Escape" {
                event.prevent_default();
                ctx.close_confirm();
            }
        })
    };

    html! {
        <dialog id="confirm-dialog" class="dialog dialog--sm" ref={dialog_ref} onkeydown={on_keydown} aria-labelledby="confirm-title">
            <div class="dialog__panel">
                <div class="dialog__head">
                    <h2 id="confirm-title">{ request.as_ref().map(|request| request.title.clone()).unwrap_or_default() }</h2>
                </div>
                <div class="dialog__body">
                    <p class="form-note">{ request.as_ref().map(|request| request.body.clone()).unwrap_or_default() }</p>
                </div>
                <div class="dialog__foot">
                    <button class="btn btn--quiet" type="button" onclick={cancel}>{ "Cancelar" }</button>
                    <button class="btn btn--danger" type="button" onclick={accept}>
                        { request.as_ref().map(|request| request.label.clone()).unwrap_or_else(|| "Confirmar".to_string()) }
                    </button>
                </div>
            </div>
        </dialog>
    }
}

pub fn key_gate(ctx: &AppCtx, title: &str, body: &str) -> Html {
    if ctx.state.settings.api_key.is_empty() {
        let ctx = ctx.clone();
        html! {
            <StatePanel
                title={title.to_string()}
                body={body.to_string()}
                icon_name="key"
                action_label="Conectar API"
                action={Callback::from(move |_: MouseEvent| ctx.open_settings())}
            />
        }
    } else {
        Html::default()
    }
}
