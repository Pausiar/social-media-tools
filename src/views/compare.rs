use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::api;
use crate::app::{key_gate, AppCtx};
use crate::components::{icon, Method, StatePanel};
use crate::format;
use crate::models::CompareEntry;
use crate::storage;
use crate::views::view_head;

const MAX_CHANNELS: usize = 3;

fn best_of(entries: &[CompareEntry], pick: impl Fn(&CompareEntry) -> f64) -> f64 {
    entries
        .iter()
        .map(pick)
        .fold(f64::NEG_INFINITY, f64::max)
}

fn metric_cell(value: String, best: bool) -> Html {
    html! {
        <td class={classes!(best.then_some("is-best"))}>{ value }</td>
    }
}

#[function_component(CompareView)]
pub fn compare_view() -> Html {
    let ctx = use_context::<AppCtx>().expect("app context");
    let input = use_state(String::new);
    let entries = use_state(storage::load_compare);
    let busy = use_state(|| false);
    let error = use_state(|| None::<String>);

    let on_input = {
        let input = input.clone();
        Callback::from(move |event: InputEvent| {
            let element = event.target_unchecked_into::<web_sys::HtmlInputElement>();
            input.set(element.value());
        })
    };

    let add = {
        let ctx = ctx.clone();
        let input = input.clone();
        let entries = entries.clone();
        let busy = busy.clone();
        let error = error.clone();
        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();
            if *busy {
                return;
            }
            let key = ctx.state.settings.api_key.clone();
            if key.is_empty() {
                ctx.open_settings();
                return;
            }
            let reference = input.trim().to_string();
            if reference.is_empty() {
                ctx.toast("Escribe el @handle o la URL de un canal");
                return;
            }
            if entries.len() >= MAX_CHANNELS {
                ctx.toast("Máximo 3 canales en la comparativa");
                return;
            }
            busy.set(true);
            error.set(None);
            let ctx_task = ctx.clone();
            let input_task = input.clone();
            let entries_task = entries.clone();
            let busy_task = busy.clone();
            let error_task = error.clone();
            let reference_task = reference.clone();
            spawn_local(async move {
                let result = api::fetch_compare(&key, &reference_task).await;
                busy_task.set(false);
                match result {
                    Ok(entry) => {
                        let mut list = (*entries_task).clone();
                        if list.iter().any(|item| item.channel.id == entry.channel.id) {
                            ctx_task.toast("Ese canal ya está en la comparativa");
                        } else {
                            list.push(entry);
                            storage::save_compare(&list);
                            entries_task.set(list);
                            input_task.set(String::new());
                            ctx_task.toast("Canal añadido a la comparativa");
                        }
                    }
                    Err(failure) => {
                        ctx_task.api_failed(&failure);
                        error_task.set(Some(failure.message));
                    }
                }
            });
        })
    };

    let remove = {
        let ctx = ctx.clone();
        let entries = entries.clone();
        Callback::from(move |id: String| {
            let mut list = (*entries).clone();
            list.retain(|entry| entry.channel.id != id);
            storage::save_compare(&list);
            entries.set(list);
            ctx.toast("Canal quitado de la comparativa");
        })
    };

    let refresh = {
        let ctx = ctx.clone();
        let entries = entries.clone();
        let busy = busy.clone();
        let error = error.clone();
        Callback::from(move |_: MouseEvent| {
            if *busy {
                return;
            }
            let key = ctx.state.settings.api_key.clone();
            if key.is_empty() {
                ctx.open_settings();
                return;
            }
            let starting = (*entries).clone();
            if starting.is_empty() {
                return;
            }
            busy.set(true);
            error.set(None);
            let ctx_task = ctx.clone();
            let entries_task = entries.clone();
            let busy_task = busy.clone();
            let error_task = error.clone();
            spawn_local(async move {
                let mut updated = Vec::with_capacity(starting.len());
                let mut failures = 0usize;
                for entry in starting.iter() {
                    match api::fetch_compare(&key, &entry.channel.id).await {
                        Ok(next) => updated.push(next),
                        Err(failure) => {
                            ctx_task.api_failed(&failure);
                            failures += 1;
                            updated.push(entry.clone());
                        }
                    }
                }
                storage::save_compare(&updated);
                entries_task.set(updated);
                busy_task.set(false);
                if failures == 0 {
                    ctx_task.toast("Comparativa actualizada");
                } else {
                    error_task.set(Some(format!(
                        "No se pudieron actualizar {} de los {} canales. Se mantienen sus últimas cifras.",
                        failures,
                        starting.len()
                    )));
                }
            });
        })
    };

    let head = view_head(
        "compare-title",
        "Comparador de canales",
        "Hasta tres canales frente a frente con las métricas que se pueden medir en público.",
        if entries.is_empty() {
            Html::default()
        } else {
            html! {
                <button class="btn btn--secondary btn--sm" type="button" onclick={refresh} disabled={*busy}>
                    { icon("refresh", "") }
                    { if *busy { "Actualizando…" } else { "Actualizar" } }
                </button>
            }
        },
    );

    if ctx.state.settings.api_key.is_empty() {
        return html! {
            <>
                { head }
                { key_gate(
                    &ctx,
                    "Conecta tu API key",
                    "El comparador consulta los canales en vivo. Sin clave no hay métricas reales que comparar.",
                ) }
            </>
        };
    }

    let full = entries.len() >= MAX_CHANNELS;
    let form = html! {
        <div class="panel">
            <div class="panel-head">
                <div>
                    <h2>{ "Añadir un canal" }</h2>
                    <p class="panel-sub">{ "Se guardan sus últimos 15 vídeos y una foto de sus métricas." }</p>
                </div>
                <span class="updated">{ format!("{}/{} canales", entries.len(), MAX_CHANNELS) }</span>
            </div>
            <form class="toolbar" onsubmit={add}>
                <div class="field field--grow">
                    <label for="compare-ref">{ "Canal de YouTube" }</label>
                    <input
                        id="compare-ref"
                        class="input"
                        type="text"
                        value={(*input).clone()}
                        oninput={on_input}
                        placeholder="@canal o youtube.com/@canal"
                        autocomplete="off"
                        spellcheck="false"
                        disabled={full}
                    />
                </div>
                <div class="field field--submit">
                    <button class="btn btn--primary" type="submit" disabled={*busy || full}>
                        { icon("plus", "") }
                        { if *busy { "Consultando…" } else { "Añadir" } }
                    </button>
                </div>
            </form>
            <p class="cost-note">
                { if full {
                    "Ya hay tres canales. Quita uno para añadir otro."
                } else {
                    "Cada canal cuesta unas 3 unidades de cuota. Actualizar los tres cuesta unas 9."
                } }
            </p>
        </div>
    };

    let banner = (*error).clone().map(|message| html! {
        <div class="notice" role="status">
            <div>
                <strong>{ "No se pudo completar la comparación." }</strong>
                <span>{ message }</span>
            </div>
        </div>
    });

    let table = if entries.is_empty() {
        html! {
            <StatePanel
                title="Todavía no hay canales"
                body="Añade hasta tres canales por su @handle o URL. Verás suscriptores, vistas medias, interacción y ritmo de subida calculados con sus últimos 15 vídeos."
                icon_name="trend"
            />
        }
    } else {
        let max_subs = best_of(&entries, |entry| {
            if entry.channel.hidden_subs {
                0.0
            } else {
                entry.channel.subs as f64
            }
        });
        let max_views = best_of(&entries, |entry| entry.channel.views as f64);
        let max_avg = best_of(&entries, |entry| entry.avg_views);
        let max_engagement = best_of(&entries, |entry| entry.engagement.unwrap_or(0.0));
        let max_cadence = best_of(&entries, |entry| entry.uploads_per_week);
        let max_last = best_of(&entries, |entry| entry.last_upload);

        let headers = entries.iter().map(|entry| {
            let remove = remove.clone();
            let id = entry.channel.id.clone();
            html! {
                <th scope="col">
                    <div class="compare-head">
                        <img src={api::safe_image(&entry.channel.avatar)} alt="" />
                        <div>
                            <a
                                href={api::channel_url(&entry.channel.id)}
                                target="_blank"
                                rel="noopener noreferrer"
                            >
                                { &entry.channel.title }
                            </a>
                            <span class="compare-head__meta">
                                { format!(
                                    "{} · {}",
                                    entry.channel.handle,
                                    format::time_ago(entry.fetched_at)
                                ) }
                            </span>
                        </div>
                        <button
                            class="icon-btn"
                            type="button"
                            aria-label={format!("Quitar «{}»", entry.channel.title)}
                            onclick={Callback::from(move |_: MouseEvent| remove.emit(id.clone()))}
                        >
                            { icon("x", "") }
                        </button>
                    </div>
                </th>
            }
        });

        let row = |label: &str, cells: Vec<Html>| {
            html! {
                <tr>
                    <th scope="row">{ label.to_string() }</th>
                    { for cells }
                </tr>
            }
        };

        let subs_cells: Vec<Html> = entries
            .iter()
            .map(|entry| {
                let best = !entry.channel.hidden_subs
                    && entry.channel.subs as f64 == max_subs
                    && max_subs > 0.0
                    && entries.len() > 1;
                let value = if entry.channel.hidden_subs {
                    "Ocultas".to_string()
                } else {
                    format::fmt_int(entry.channel.subs)
                };
                metric_cell(value, best)
            })
            .collect();
        let views_cells: Vec<Html> = entries
            .iter()
            .map(|entry| {
                let best = entry.channel.views as f64 == max_views && max_views > 0.0 && entries.len() > 1;
                metric_cell(format::fmt_compact(entry.channel.views), best)
            })
            .collect();
        let avg_cells: Vec<Html> = entries
            .iter()
            .map(|entry| {
                let best = entry.avg_views == max_avg && max_avg > 0.0 && entries.len() > 1;
                metric_cell(format::fmt_compact(entry.avg_views.round() as u64), best)
            })
            .collect();
        let engagement_cells: Vec<Html> = entries
            .iter()
            .map(|entry| {
                let value = entry.engagement.unwrap_or(0.0);
                let best = value == max_engagement && max_engagement > 0.0 && entries.len() > 1;
                metric_cell(
                    entry
                        .engagement
                        .map(|item| format::fmt_percent(item, 2))
                        .unwrap_or_else(|| "—".into()),
                    best,
                )
            })
            .collect();
        let cadence_cells: Vec<Html> = entries
            .iter()
            .map(|entry| {
                let best = entry.uploads_per_week == max_cadence
                    && max_cadence > 0.0
                    && entries.len() > 1;
                metric_cell(
                    format!("{} /semana", format::fmt_float(entry.uploads_per_week, 1)),
                    best,
                )
            })
            .collect();
        let last_cells: Vec<Html> = entries
            .iter()
            .map(|entry| {
                let best = entry.last_upload == max_last && max_last > 0.0 && entries.len() > 1;
                metric_cell(format::fmt_date(entry.last_upload), best)
            })
            .collect();
        let videos_cells: Vec<Html> = entries
            .iter()
            .map(|entry| metric_cell(format::fmt_int(entry.channel.video_count), false))
            .collect();

        html! {
            <div class="panel">
                <div class="panel-head">
                    <div>
                        <h2>{ "Comparativa" }</h2>
                        <p class="panel-sub">{ "La mejor cifra de cada fila se marca en rojo. Los datos son la última foto guardada." }</p>
                    </div>
                </div>
                <div class="table-wrap">
                    <table class="data-table compare-table">
                        <thead>
                            <tr>
                                <th scope="col"><span class="sr-only">{ "Métrica" }</span></th>
                                { for headers }
                            </tr>
                        </thead>
                        <tbody>
                            { row("Suscriptores", subs_cells) }
                            { row("Vistas totales", views_cells) }
                            { row("Vistas medias (15)", avg_cells) }
                            { row("Interacción mediana", engagement_cells) }
                            { row("Ritmo de subida", cadence_cells) }
                            { row("Última subida", last_cells) }
                            { row("Vídeos públicos", videos_cells) }
                        </tbody>
                    </table>
                </div>
                <Method summary="Qué se está midiendo">
                    <p>
                        { "Suscriptores, vistas totales y número de vídeos son los que publica el canal. Las vistas medias y la interacción mediana se calculan con sus últimos 15 vídeos públicos; el ritmo de subida sale de las fechas de esos mismos vídeos." }
                    </p>
                    <p>
                        { "La interacción es (likes + comentarios) ÷ vistas del vídeo, y se usa la mediana para que un vídeo viral no distorsione la comparación. «Ocultas» significa que el canal no publica su número de suscriptores." }
                    </p>
                </Method>
            </div>
        }
    };

    html! {
        <>
            { head }
            { form }
            { banner.unwrap_or_default() }
            { table }
        </>
    }
}
