use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::analysis;
use crate::api;
use crate::app::{key_gate, AppCtx};
use crate::components::{icon, LoadingPanel, Method, StatItem, StatStrip, StatePanel};
use crate::format;
use crate::models::Video;
use crate::views::{external_link, view_head, video_cell};

const COST_UNITS: u32 = 101;

#[derive(Clone, PartialEq)]
struct TrendData {
    query: String,
    window_days: u32,
    videos: Vec<Video>,
    fetched_at: f64,
}

fn velocity(video: &Video) -> f64 {
    let published = video.published_ms();
    if !published.is_finite() {
        return 0.0;
    }
    let hours = ((format::now_ms() - published) / 3_600_000.0).max(1.0);
    video.views as f64 / hours
}

#[function_component(TrendsView)]
pub fn trends_view() -> Html {
    let ctx = use_context::<AppCtx>().expect("app context");
    let query = use_state(String::new);
    let window_days = use_state(|| 30u32);
    let duration = use_state(|| "any".to_string());
    let data = use_state(|| None::<TrendData>);
    let error = use_state(|| None::<String>);
    let busy = use_state(|| false);

    let on_query = {
        let query = query.clone();
        Callback::from(move |event: InputEvent| {
            let element = event.target_unchecked_into::<web_sys::HtmlInputElement>();
            query.set(element.value());
        })
    };

    let on_window = {
        let window_days = window_days.clone();
        Callback::from(move |event: Event| {
            let element = event.target_unchecked_into::<web_sys::HtmlSelectElement>();
            window_days.set(element.value().parse().unwrap_or(30));
        })
    };

    let on_duration = {
        let duration = duration.clone();
        Callback::from(move |event: Event| {
            let element = event.target_unchecked_into::<web_sys::HtmlInputElement>();
            duration.set(element.value());
        })
    };

    let run_search = {
        let ctx = ctx.clone();
        let query = query.clone();
        let window_days = window_days.clone();
        let duration = duration.clone();
        let data = data.clone();
        let error = error.clone();
        let busy = busy.clone();
        Callback::from(move |_: ()| {
            if *busy {
                return;
            }
            let key = ctx.state.settings.api_key.clone();
            if key.is_empty() {
                ctx.open_settings();
                return;
            }
            let term = query.trim().to_string();
            if term.is_empty() {
                ctx.toast("Escribe un tema para buscar");
                return;
            }
            let region = ctx.state.settings.region.clone();
            let window_value = *window_days;
            let duration_value = (*duration).clone();
            busy.set(true);
            error.set(None);
            let ctx_task = ctx.clone();
            let data_task = data.clone();
            let error_task = error.clone();
            let busy_task = busy.clone();
            spawn_local(async move {
                let result = api::fetch_trends(
                    &key,
                    &term,
                    window_value,
                    &duration_value,
                    &region,
                )
                .await;
                busy_task.set(false);
                match result {
                    Ok(payload) => {
                        data_task.set(Some(TrendData {
                            query: payload.query,
                            window_days: window_value,
                            videos: payload.videos,
                            fetched_at: format::now_ms(),
                        }));
                    }
                    Err(failure) => {
                        ctx_task.api_failed(&failure);
                        error_task.set(Some(failure.message));
                    }
                }
            });
        })
    };

    let on_submit = {
        let run_search = run_search.clone();
        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();
            run_search.emit(());
        })
    };

    let retry = {
        let run_search = run_search.clone();
        Callback::from(move |_: MouseEvent| run_search.emit(()))
    };

    let head = view_head(
        "trends-title",
        "Radar de tendencias",
        "Los 25 vídeos que más crecen en un tema y una ventana de tiempo, con su velocidad real.",
        Html::default(),
    );

    if ctx.state.settings.api_key.is_empty() {
        return html! {
            <>
                { head }
                { key_gate(
                    &ctx,
                    "Conecta tu API key",
                    "El radar consulta búsquedas reales de YouTube. Sin clave no hay resultados que mostrar.",
                ) }
            </>
        };
    }

    let form = html! {
        <div class="panel">
            <div class="panel-head">
                <div>
                    <h2>{ "Buscar una tendencia" }</h2>
                    <p class="panel-sub">{ "Se ordena por vistas y se mide la velocidad con la fecha de publicación." }</p>
                </div>
                <span class="updated">{ format!("Región {}", ctx.state.settings.region) }</span>
            </div>
            <form class="stack-form" onsubmit={on_submit}>
                <div class="toolbar">
                    <div class="field field--grow">
                        <label for="trend-query">{ "Tema o keyword" }</label>
                        <input
                            id="trend-query"
                            class="input"
                            type="text"
                            value={(*query).clone()}
                            oninput={on_query}
                            placeholder="por ejemplo, rutina de mañana"
                            autocomplete="off"
                            spellcheck="false"
                        />
                    </div>
                    <div class="field">
                        <label for="trend-window">{ "Ventana" }</label>
                        <select class="input" id="trend-window" onchange={on_window}>
                            <option value="7" selected={*window_days == 7}>{ "7 días" }</option>
                            <option value="30" selected={*window_days == 30}>{ "30 días" }</option>
                            <option value="90" selected={*window_days == 90}>{ "90 días" }</option>
                            <option value="365" selected={*window_days == 365}>{ "1 año" }</option>
                        </select>
                    </div>
                    <div class="field field--submit">
                        <button class="btn btn--primary" type="submit" disabled={*busy}>
                            { icon("search", "") }
                            { if *busy { "Buscando…" } else { "Buscar" } }
                        </button>
                    </div>
                </div>
                <div class="times-run">
                    <div class="field">
                        <label>{ "Duración" }</label>
                        <div class="segmented">
                            <label class="segmented__item">
                                <input type="radio" name="trend-duration" value="any" checked={*duration == "any"} onchange={on_duration.clone()} />
                                <span>{ "Cualquiera" }</span>
                            </label>
                            <label class="segmented__item">
                                <input type="radio" name="trend-duration" value="short" checked={*duration == "short"} onchange={on_duration.clone()} />
                                <span>{ "Corto" }</span>
                            </label>
                            <label class="segmented__item">
                                <input type="radio" name="trend-duration" value="medium" checked={*duration == "medium"} onchange={on_duration.clone()} />
                                <span>{ "Medio" }</span>
                            </label>
                            <label class="segmented__item">
                                <input type="radio" name="trend-duration" value="long" checked={*duration == "long"} onchange={on_duration.clone()} />
                                <span>{ "Largo" }</span>
                            </label>
                        </div>
                    </div>
                    <span class="cost-note">
                        { format!("Cada búsqueda cuesta unas {COST_UNITS} unidades de cuota de las 10.000 diarias.") }
                    </span>
                </div>
            </form>
        </div>
    };

    let results = if *busy && data.is_none() {
        html! { <LoadingPanel label="Buscando tendencias…" /> }
    } else if let Some(message) = (*error).clone() {
        html! {
            <StatePanel
                title="La búsqueda no se completó"
                body={message}
                error=true
                icon_name="alert"
                action_label="Reintentar"
                action={retry.clone()}
            />
        }
    } else if let Some(current) = (*data).clone() {
        trend_results(&current)
    } else {
        html! {
            <StatePanel
                title="Sin búsqueda todavía"
                body="Escribe un tema y elige una ventana. Verás los vídeos con más vistas, su velocidad en vistas por hora y qué títulos se repiten."
                icon_name="trend"
            />
        }
    };

    html! {
        <>
            { head }
            { form }
            { results }
        </>
    }
}

fn trend_results(data: &TrendData) -> Html {
    let videos = &data.videos;
    if videos.is_empty() {
        return html! {
            <StatePanel
                title="Sin resultados en esa ventana"
                body="Prueba una ventana más amplia, quita el filtro de duración o cambia el tema."
                icon_name="search"
            />
        };
    }

    let views: Vec<f64> = videos.iter().map(|video| video.views as f64).collect();
    let median_views = analysis::median(&views).unwrap_or(0.0);
    let outlier_line = median_views * 2.0;
    let outliers = videos
        .iter()
        .filter(|video| video.views as f64 >= outlier_line)
        .count();
    let velocities: Vec<f64> = videos.iter().map(velocity).collect();
    let median_velocity = analysis::median(&velocities).unwrap_or(0.0);
    let top_velocity = velocities.iter().copied().fold(0.0f64, f64::max);
    let total_views: u64 = videos.iter().map(|video| video.views).sum();

    let stats = html! {
        <StatStrip items={vec![
            StatItem {
                label: "Vídeos analizados".into(),
                value: format::fmt_int(videos.len() as u64),
                delta: Some(format!(
                    "Ventana de {} días · {} vistas en total",
                    data.window_days,
                    format::fmt_compact(total_views)
                )),
            },
            StatItem {
                label: "Vistas medianas".into(),
                value: format::fmt_compact(median_views.round() as u64),
                delta: Some(format!(
                    "Destacado desde: {} vistas",
                    format::fmt_compact(outlier_line.round() as u64)
                )),
            },
            StatItem {
                label: "Destacados".into(),
                value: format::fmt_int(outliers as u64),
                delta: Some("Al menos el doble de la mediana".into()),
            },
            StatItem {
                label: "Velocidad mediana".into(),
                value: format!("{} /h", format::fmt_compact(median_velocity.round() as u64)),
                delta: Some(format!(
                    "La más rápida: {} vistas/hora",
                    format::fmt_compact(top_velocity.round() as u64)
                )),
            },
        ]} />
    };

    let top_term = analysis::extract_terms(
        &videos.iter().map(|video| video.title.clone()).collect::<Vec<_>>(),
        12,
    );
    let hashtags = analysis::extract_hashtags(
        &videos.iter().map(|video| video.title.clone()).collect::<Vec<_>>(),
        8,
    );

    let mut rows: Vec<&Video> = videos.iter().collect();
    rows.sort_by(|a, b| velocity(b).total_cmp(&velocity(a)));

    let table_rows = rows.iter().map(|video| {
        let signal = if video.views as f64 >= outlier_line && outlier_line > 0.0 {
            html! { <span class="badge badge--red">{ "Destacado" }</span> }
        } else if format::now_ms() - video.published_ms() < 72.0 * 3_600_000.0 {
            html! { <span class="badge">{ "Reciente" }</span> }
        } else {
            html! { <span class="badge">{ "—" }</span> }
        };
        let meta = format!(
            "{} · {}",
            video.channel_title,
            format::fmt_date(video.published_ms())
        );
        html! {
            <tr>
                <td>{ video_cell(video, meta) }</td>
                <td class="num" title={format::fmt_int(video.views)}>{ format::fmt_compact(video.views) }</td>
                <td class="num">{ format::fmt_compact(velocity(video).round() as u64) }</td>
                <td>{ signal }</td>
                <td class="center">{ external_link(&video.id, &video.title) }</td>
            </tr>
        }
    });

    let terms_html = if top_term.is_empty() {
        html! { <p class="empty-inline">{ "Sin términos repetidos: los títulos de esta búsqueda no comparten palabras." }</p> }
    } else {
        html! {
            <div class="chip-row">
                { for top_term.iter().map(|term| html! {
                    <span class="chip">
                        <span class="chip__name">{ &term.term }</span>
                        <span class="badge">{ term.count }</span>
                    </span>
                }) }
            </div>
        }
    };

    let hashtags_html = if hashtags.is_empty() {
        html! { <p class="empty-inline">{ "Ningún título visible incluye hashtags." }</p> }
    } else {
        html! {
            <div class="chip-row">
                { for hashtags.iter().map(|tag| html! {
                    <span class="tag">{ &tag.term }</span>
                }) }
            </div>
        }
    };

    let summary = format!(
        "Resultados para «{}» en los últimos {} días, ordenados por velocidad.",
        data.query, data.window_days
    );

    html! {
        <div class="results">
            <div class="panel">
                <div class="panel-head">
                    <div>
                        <h2>{ "Resumen de la búsqueda" }</h2>
                        <p class="panel-sub">{ summary }</p>
                    </div>
                    <span class="updated">{ format!("Actualizado {}", format::time_ago(data.fetched_at)) }</span>
                </div>
                { stats }
                <div class="insights">
                    <p class="insight">
                        { "El vídeo mediano acumula " }
                        <strong>{ format!("{} vistas", format::fmt_compact(median_views.round() as u64)) }</strong>
                        { " y " }
                        <strong>{ format!("{} vistas por hora", format::fmt_compact(median_velocity.round() as u64)) }</strong>
                        { " desde que se publicó." }
                    </p>
                    <p class="insight">
                        { "Hay " }
                        <strong>{ format::plural(outliers as u64, "vídeo destacado", "vídeos destacados") }</strong>
                        { " por encima del doble de la mediana. Son los temas que están funcionando ahora, no los que funcionaron hace meses." }
                    </p>
                    if let Some(term) = top_term.first() {
                        <p class="insight">
                            { "El término más repetido en los títulos es " }
                            <strong>{ format!("«{}»", term.term) }</strong>
                            { format!(", presente en {} de los {} vídeos.", term.count, videos.len()) }
                        </p>
                    }
                </div>
            </div>

            <div class="panel">
                <div class="panel-head">
                    <div>
                        <h2>{ "Vídeos de la ventana" }</h2>
                        <p class="panel-sub">{ "Ordenados por vistas por hora desde su publicación." }</p>
                    </div>
                </div>
                <div class="table-wrap">
                    <table class="data-table">
                        <thead>
                            <tr>
                                <th scope="col">{ "Vídeo" }</th>
                                <th scope="col" class="num">{ "Vistas" }</th>
                                <th scope="col" class="num">{ "Vistas/h" }</th>
                                <th scope="col">{ "Señal" }</th>
                                <th scope="col"><span class="sr-only">{ "Abrir" }</span></th>
                            </tr>
                        </thead>
                        <tbody>
                            { for table_rows }
                        </tbody>
                    </table>
                </div>
            </div>

            <div class="panel">
                <div class="panel-head">
                    <div>
                        <h2>{ "Patrones en los títulos" }</h2>
                        <p class="panel-sub">{ "Lo que ya se repite en los resultados, sin sugerencias inventadas." }</p>
                    </div>
                </div>
                <div class="stack">
                    <div>
                        <h3>{ "Términos repetidos" }</h3>
                        { terms_html }
                    </div>
                    <div>
                        <h3>{ "Hashtags en los títulos" }</h3>
                        { hashtags_html }
                    </div>
                </div>
                <Method summary="Cómo se calcula">
                    <p>
                        { "Search de YouTube con orden por vistas, ventana de publicación y filtro de duración. Las vistas y fechas se piden a la API de vídeos; la velocidad es vistas divididas entre las horas transcurridas, con un mínimo de una hora." }
                    </p>
                    <p>
                        { "Un vídeo destacado tiene al menos el doble de vistas que la mediana de la búsqueda. Los términos excluyen palabras vacías y solo aparecen si se repiten al menos dos veces." }
                    </p>
                </Method>
            </div>
        </div>
    }
}
