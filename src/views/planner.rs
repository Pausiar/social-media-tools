use wasm_bindgen::{JsCast, JsValue};
use yew::prelude::*;

use crate::app::AppCtx;
use crate::components::icon;
use crate::format;
use crate::models::Post;
use crate::storage;
use crate::views::view_head;

fn platform_label(value: &str) -> &'static str {
    match value {
        "tiktok" => "TikTok",
        "shorts" => "Shorts",
        _ => "YouTube",
    }
}

fn platform_from(value: &str) -> &'static str {
    match value {
        "tiktok" => "tiktok",
        "shorts" => "shorts",
        _ => "youtube",
    }
}

fn sort_posts(posts: &mut [Post]) {
    posts.sort_by(|a, b| a.at.total_cmp(&b.at));
}

fn day_month(at: f64) -> (String, String) {
    let value = format::fmt_day_month(at);
    match value.split_once(' ') {
        Some((day, month)) => (day.to_string(), month.to_string()),
        None => (value, String::new()),
    }
}

fn ics_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace("\r\n", "\\n")
        .replace('\n', "\\n")
}

fn ics_stamp(ms: f64) -> String {
    let date = js_sys::Date::new(&JsValue::from_f64(ms));
    format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}Z",
        date.get_utc_full_year(),
        date.get_utc_month() + 1,
        date.get_utc_date(),
        date.get_utc_hours(),
        date.get_utc_minutes(),
        date.get_utc_seconds()
    )
}

fn download_calendar(posts: &[Post]) {
    let stamp = ics_stamp(format::now_ms());
    let mut lines = vec![
        "BEGIN:VCALENDAR".to_string(),
        "VERSION:2.0".to_string(),
        "PRODID:-//Pausiar Tools//Planificador//ES".to_string(),
        "CALSCALE:GREGORIAN".to_string(),
        "METHOD:PUBLISH".to_string(),
    ];
    for post in posts {
        let description = if post.note.is_empty() {
            platform_label(&post.platform).to_string()
        } else {
            format!("{} · {}", platform_label(&post.platform), post.note)
        };
        lines.push("BEGIN:VEVENT".to_string());
        lines.push(format!("UID:{}@pausiar.tools", post.id));
        lines.push(format!("DTSTAMP:{stamp}"));
        lines.push(format!("DTSTART:{}", ics_stamp(post.at)));
        lines.push(format!("DTEND:{}", ics_stamp(post.at + 3_600_000.0)));
        lines.push(format!("SUMMARY:{}", ics_escape(&post.title)));
        lines.push(format!("DESCRIPTION:{}", ics_escape(&description)));
        lines.push("END:VEVENT".to_string());
    }
    lines.push("END:VCALENDAR".to_string());
    let content = format!("{}\r\n", lines.join("\r\n"));

    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    let options = web_sys::BlobPropertyBag::new();
    options.set_type("text/calendar;charset=utf-8");
    let parts = js_sys::Array::new();
    parts.push(&JsValue::from_str(&content));
    let Ok(blob) = web_sys::Blob::new_with_str_sequence_and_options(&parts, &options) else {
        return;
    };
    let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) else {
        return;
    };
    if let Ok(element) = document.create_element("a") {
        let _ = element.set_attribute("href", &url);
        let _ = element.set_attribute("download", "pausiar-plan.ics");
        if let Ok(anchor) = element.dyn_into::<web_sys::HtmlElement>() {
            anchor.click();
        }
    }
    let _ = web_sys::Url::revoke_object_url(&url);
}

#[function_component(PlannerView)]
pub fn planner_view() -> Html {
    let ctx = use_context::<AppCtx>().expect("app context");
    let posts = use_state(storage::load_posts);
    let title = use_state(String::new);
    let platform = use_state(|| "youtube".to_string());
    let at = use_state(|| format::fmt_datetime_local_value(format::next_hour(format::now_ms())));
    let note = use_state(String::new);
    let editing = use_state(|| None::<String>);
    let last_deleted = use_state(|| None::<Post>);
    let error = use_state(|| None::<String>);

    let head = view_head(
        "planner-title",
        "Planificador",
        "Organiza qué publicas y cuándo. Se guarda en este navegador y se exporta a tu calendario.",
        Html::default(),
    );

    let on_title = {
        let title = title.clone();
        Callback::from(move |event: InputEvent| {
            let element = event.target_unchecked_into::<web_sys::HtmlInputElement>();
            title.set(element.value());
        })
    };
    let on_note = {
        let note = note.clone();
        Callback::from(move |event: InputEvent| {
            let element = event.target_unchecked_into::<web_sys::HtmlInputElement>();
            note.set(element.value());
        })
    };
    let on_at = {
        let at = at.clone();
        Callback::from(move |event: InputEvent| {
            let element = event.target_unchecked_into::<web_sys::HtmlInputElement>();
            at.set(element.value());
        })
    };
    let on_platform = {
        let platform = platform.clone();
        Callback::from(move |event: Event| {
            let element = event.target_unchecked_into::<web_sys::HtmlSelectElement>();
            platform.set(platform_from(&element.value()).to_string());
        })
    };

    let reset = {
        let title = title.clone();
        let note = note.clone();
        let at = at.clone();
        let editing = editing.clone();
        let error = error.clone();
        Callback::from(move |_: ()| {
            title.set(String::new());
            note.set(String::new());
            at.set(format::fmt_datetime_local_value(format::next_hour(format::now_ms())));
            editing.set(None);
            error.set(None);
        })
    };

    let submit = {
        let ctx = ctx.clone();
        let posts = posts.clone();
        let title = title.clone();
        let platform = platform.clone();
        let at = at.clone();
        let note = note.clone();
        let editing = editing.clone();
        let error = error.clone();
        let reset = reset.clone();
        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();
            let clean_title = title.trim().to_string();
            if clean_title.is_empty() {
                error.set(Some("Escribe un título o una idea para la publicación.".into()));
                return;
            }
            let Some(moment) = format::parse_datetime_local(&at) else {
                error.set(Some("Elige una fecha y una hora válidas.".into()));
                return;
            };
            let mut list = (*posts).clone();
            if let Some(id) = (*editing).clone() {
                if let Some(post) = list.iter_mut().find(|post| post.id == id) {
                    post.title = clean_title;
                    post.platform = (*platform).clone();
                    post.at = moment;
                    post.note = note.trim().to_string();
                }
                ctx.toast("Publicación actualizada");
            } else {
                list.push(Post {
                    id: format::new_id(),
                    title: clean_title,
                    platform: (*platform).clone(),
                    at: moment,
                    note: note.trim().to_string(),
                    done: false,
                });
                ctx.toast("Publicación añadida al plan");
            }
            sort_posts(&mut list);
            storage::save_posts(&list);
            posts.set(list);
            reset.emit(());
        })
    };

    let cancel_edit = {
        let reset = reset.clone();
        Callback::from(move |_: MouseEvent| reset.emit(()))
    };

    let toggle = {
        let posts = posts.clone();
        Callback::from(move |id: String| {
            let mut list = (*posts).clone();
            if let Some(post) = list.iter_mut().find(|post| post.id == id) {
                post.done = !post.done;
            }
            storage::save_posts(&list);
            posts.set(list);
        })
    };

    let remove = {
        let ctx = ctx.clone();
        let posts = posts.clone();
        let last_deleted = last_deleted.clone();
        let editing = editing.clone();
        Callback::from(move |id: String| {
            let mut list = (*posts).clone();
            if let Some(index) = list.iter().position(|post| post.id == id) {
                let removed = list.remove(index);
                if (*editing).as_deref() == Some(removed.id.as_str()) {
                    editing.set(None);
                }
                storage::save_posts(&list);
                posts.set(list);
                last_deleted.set(Some(removed));
                ctx.toast("Publicación eliminada");
            }
        })
    };

    let undo = {
        let ctx = ctx.clone();
        let posts = posts.clone();
        let last_deleted = last_deleted.clone();
        Callback::from(move |_: MouseEvent| {
            let Some(post) = (*last_deleted).clone() else {
                return;
            };
            let mut list = (*posts).clone();
            if !list.iter().any(|item| item.id == post.id) {
                list.push(post);
                sort_posts(&mut list);
                storage::save_posts(&list);
                posts.set(list);
            }
            last_deleted.set(None);
            ctx.toast("Publicación restaurada");
        })
    };

    let start_edit = {
        let title = title.clone();
        let platform = platform.clone();
        let at = at.clone();
        let note = note.clone();
        let editing = editing.clone();
        let error = error.clone();
        Callback::from(move |post: Post| {
            title.set(post.title);
            platform.set(post.platform);
            at.set(format::fmt_datetime_local_value(post.at));
            note.set(post.note);
            editing.set(Some(post.id));
            error.set(None);
        })
    };

    let export = {
        let ctx = ctx.clone();
        let posts = posts.clone();
        Callback::from(move |_: MouseEvent| {
            let pending: Vec<Post> = (*posts)
                .iter()
                .filter(|post| !post.done)
                .cloned()
                .collect();
            if pending.is_empty() {
                ctx.toast("No hay publicaciones pendientes que exportar");
                return;
            }
            download_calendar(&pending);
            ctx.toast("Calendario exportado");
        })
    };

    let now = format::now_ms();
    let render_group = |label: &str, items: Vec<Post>| -> Html {
        if items.is_empty() {
            return Html::default();
        }
        let count = items.len();
        let rows = items.into_iter().map(|post| {
            let (day, month) = day_month(post.at);
            let overdue = !post.done && post.at < now;
            let toggle = toggle.clone();
            let remove = remove.clone();
            let start_edit = start_edit.clone();
            let toggle_id = post.id.clone();
            let remove_id = post.id.clone();
            let edit_post = post.clone();
            html! {
                <div class={classes!("post-row", post.done.then_some("is-done"))}>
                    <div class="post-date">
                        <span class="post-day">{ day }</span>
                        <span class="post-month">{ month }</span>
                    </div>
                    <div class="post-main">
                        <div class="post-title">{ &post.title }</div>
                        <div class="post-meta">
                            <span class="badge">{ platform_label(&post.platform) }</span>
                            <span>{ format::fmt_datetime(post.at) }</span>
                            if overdue {
                                <span class="badge badge--red">{ "Atrasada" }</span>
                            }
                            if !post.note.is_empty() {
                                <span class="post-note">{ &post.note }</span>
                            }
                        </div>
                    </div>
                    <div class="post-actions">
                        <button
                            class={classes!("icon-btn", post.done.then_some("is-on"))}
                            type="button"
                            aria-label={if post.done { "Marcar como pendiente" } else { "Marcar como hecha" }}
                            aria-pressed={post.done.to_string()}
                            onclick={Callback::from(move |_: MouseEvent| toggle.emit(toggle_id.clone()))}
                        >
                            { icon("check", "") }
                        </button>
                        <button
                            class="icon-btn"
                            type="button"
                            aria-label="Editar publicación"
                            onclick={Callback::from(move |_: MouseEvent| start_edit.emit(edit_post.clone()))}
                        >
                            { icon("edit", "") }
                        </button>
                        <button
                            class="icon-btn"
                            type="button"
                            aria-label="Eliminar publicación"
                            onclick={Callback::from(move |_: MouseEvent| remove.emit(remove_id.clone()))}
                        >
                            { icon("trash", "") }
                        </button>
                    </div>
                </div>
            }
        });
        html! {
            <div class="post-group">
                <div class="post-group__head">
                    <span>{ label.to_string() }</span>
                    <span>{ format::plural(count as u64, "publicación", "publicaciones") }</span>
                </div>
                { for rows }
            </div>
        }
    };

    let mut pending: Vec<Post> = posts.iter().filter(|post| !post.done).cloned().collect();
    let mut done: Vec<Post> = posts.iter().filter(|post| post.done).cloned().collect();
    sort_posts(&mut pending);
    sort_posts(&mut done);

    let list = if posts.is_empty() {
        html! {
            <div class="empty-inline">
                <p>{ "Aún no hay publicaciones. Añade la primera a la izquierda y quedará guardada en este navegador." }</p>
            </div>
        }
    } else {
        html! {
            <>
                { render_group("Pendientes", pending) }
                { render_group("Hechas", done) }
            </>
        }
    };

    let editing_now = editing.is_some();
    let banner = last_deleted.as_ref().map(|post| {
        let undo = undo.clone();
        html! {
            <div class="notice" role="status">
                <div>
                    <strong>{ "Publicación eliminada." }</strong>
                    <span>{ &post.title }</span>
                </div>
                <button class="btn btn--secondary btn--sm" type="button" onclick={undo}>
                    { "Deshacer" }
                </button>
            </div>
        }
    });

    html! {
        <>
            { head }
            { banner.unwrap_or_default() }
            <div class="planner-grid">
                <div class="panel">
                    <div class="panel-head">
                        <div>
                            <h2>{ if editing_now { "Editar publicación" } else { "Nueva publicación" } }</h2>
                            <p class="panel-sub">{ "Sirve para YouTube, Shorts y TikTok; no publica nada por sí solo." }</p>
                        </div>
                    </div>
                    <form class="stack-form" onsubmit={submit}>
                        <div class="field">
                            <label for="post-title">{ "Título o idea" }</label>
                            <input
                                id="post-title"
                                class="input"
                                type="text"
                                value={(*title).clone()}
                                oninput={on_title}
                                placeholder="por ejemplo, tutorial de edición en 60 s"
                                autocomplete="off"
                                spellcheck="false"
                            />
                        </div>
                        <div class="form-two">
                            <div class="field">
                                <label for="post-platform">{ "Plataforma" }</label>
                                <select class="input" id="post-platform" onchange={on_platform}>
                                    <option value="youtube" selected={*platform == "youtube"}>{ "YouTube" }</option>
                                    <option value="shorts" selected={*platform == "shorts"}>{ "Shorts" }</option>
                                    <option value="tiktok" selected={*platform == "tiktok"}>{ "TikTok" }</option>
                                </select>
                            </div>
                            <div class="field">
                                <label for="post-at">{ "Fecha y hora" }</label>
                                <input
                                    id="post-at"
                                    class="input"
                                    type="datetime-local"
                                    value={(*at).clone()}
                                    oninput={on_at}
                                />
                            </div>
                        </div>
                        <div class="field">
                            <label for="post-note">{ "Nota " }<span class="label-hint">{ "opcional" }</span></label>
                            <input
                                id="post-note"
                                class="input"
                                type="text"
                                value={(*note).clone()}
                                oninput={on_note}
                                placeholder="gancho, formato o recordatorio"
                                autocomplete="off"
                            />
                        </div>
                        if let Some(message) = &*error {
                            <p class="form-error">{ message }</p>
                        }
                        <div class="form-actions">
                            <button class="btn btn--primary" type="submit">
                                { icon("plus", "") }
                                { if editing_now { "Guardar cambios" } else { "Añadir al plan" } }
                            </button>
                            if editing_now {
                                <button class="btn btn--quiet" type="button" onclick={cancel_edit}>
                                    { "Cancelar" }
                                </button>
                            }
                        </div>
                    </form>
                    <p class="panel-note">
                        { "La fecha se interpreta en tu zona horaria. El .ics se exporta en UTC para que el calendario lo coloque bien." }
                    </p>
                </div>

                <div class="panel">
                    <div class="panel-head">
                        <div>
                            <h2>{ "Plan" }</h2>
                            <p class="panel-sub">{ "Las hechas se quedan como historial; las atrasadas se marcan solas." }</p>
                        </div>
                        <button class="btn btn--secondary btn--sm" type="button" onclick={export} disabled={posts.is_empty()}>
                            { icon("download", "") }
                            { "Exportar .ics" }
                        </button>
                    </div>
                    { list }
                </div>
            </div>
        </>
    }
}
