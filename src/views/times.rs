use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::analysis;
use crate::api;
use crate::app::{key_gate, AppCtx};
use crate::components::{icon, LoadingPanel, Method, StatItem, StatStrip, StatePanel};
use crate::format;
use crate::models::{SourceRef, Video};
use crate::storage;
use crate::views::view_head;

const MAX_SOURCES: usize = 3;
const BLOCKS: [&str; 4] = ["00–06", "06–12", "12–18", "18–24"];

#[derive(Clone, PartialEq)]
struct TimeData {
    videos: Vec<Video>,
    analyzed_at: f64,
    source_count: usize,
}

#[derive(Clone, PartialEq)]
struct Cell {
    count: usize,
    level: u8,
    index: f64,
}

fn level_for(index: f64) -> u8 {
    if index >= 1.8 {
        5
    } else if index >= 1.2 {
        4
    } else if index >= 0.9 {
        3
    } else if index >= 0.5 {
        2
    } else {
        1
    }
}

fn build_cells(videos: &[Video]) -> Vec<Cell> {
    let mut views: Vec<Vec<f64>> = vec![Vec::new(); 28];
    for video in videos {
        let published = video.published_ms();
        if !published.is_finite() {
            continue;
        }
        let weekday = format::weekday_index(published);
        let block = (format::hour_of_day(published) / 6) as usize;
        views[weekday * 4 + block.min(3)].push(video.views as f64);
    }
    let overall = analysis::median(&views.iter().flatten().copied().collect::<Vec<_>>()).unwrap_or(0.0);
    views
        .into_iter()
        .map(|bucket| {
            let count = bucket.len();
            if count == 0 || overall <= 0.0 {
                return Cell {
                    count,
                    level: 0,
                    index: 0.0,
                };
            }
            let median = analysis::median(&bucket).unwrap_or(0.0);
            let index = median / overall;
            Cell {
                count,
                level: level_for(index),
                index,
            }
        })
        .collect()
}

#[function_component(TimesView)]
pub fn times_view() -> Html {
    let ctx = use_context::<AppCtx>().expect("app context");
    let input = use_state(String::new);
    let sources = use_state(storage::load_sources);
    let limit = use_state(|| 25u32);
    let data = use_state(|| None::<TimeData>);
    let busy = use_state(|| false);
    let adding = use_state(|| false);
    let error = use_state(|| None::<String>);

    let on_input = {
        let input = input.clone();
        Callback::from(move |event: InputEvent| {
            let element = event.target_unchecked_into::<web_sys::HtmlInputElement>();
            input.set(element.value());
        })
    };

    let add_source = {
        let ctx = ctx.clone();
        let input = input.clone();
        let sources = sources.clone();
        let data = data.clone();
        let busy = busy.clone();
        let adding = adding.clone();
        let error = error.clone();
        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();
            if *busy || *adding {
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
            if sources.len() >= MAX_SOURCES {
                ctx.toast("Máximo 3 canales");
                return;
            }
            adding.set(true);
            error.set(None);
            let ctx_task = ctx.clone();
            let input_task = input.clone();
            let sources_task = sources.clone();
            let data_task = data.clone();
            let adding_task = adding.clone();
            let error_task = error.clone();
            spawn_local(async move {
                let result = api::resolve_channel(&key, &reference).await;
                adding_task.set(false);
                match result {
                    Ok(channel) => {
                        let mut list = (*sources_task).clone();
                        if list.iter().any(|source| source.id == channel.id) {
                            ctx_task.toast("Ese canal ya está en la lista");
                            return;
                        }
                        list.push(SourceRef {
                            id: channel.id,
                            title: channel.title,
                            avatar: channel.avatar,
                            handle: channel.handle,
                        });
                        storage::save_sources(&list);
                        sources_task.set(list);
                        data_task.set(None);
                        input_task.set(String::new());
                        ctx_task.toast("Canal añadido");
                    }
                    Err(failure) => {
                        ctx_task.api_failed(&failure);
                        error_task.set(Some(failure.message));
                    }
                }
            });
        })
    };

    let remove_source = {
        let ctx = ctx.clone();
        let sources = sources.clone();
        let data = data.clone();
        Callback::from(move |id: String| {
            let mut list = (*sources).clone();
            list.retain(|source| source.id != id);
            storage::save_sources(&list);
            sources.set(list);
            data.set(None);
            ctx.toast("Canal quitado");
        })
    };

    let analyze = {
        let ctx = ctx.clone();
        let sources = sources.clone();
        let limit = limit.clone();
        let data = data.clone();
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
            let list = (*sources).clone();
            if list.is_empty() {
                ctx.toast("Añade al menos un canal");
                return;
            }
            let per_channel = *limit as usize;
            busy.set(true);
            error.set(None);
            let ctx_task = ctx.clone();
            let data_task = data.clone();
            let busy_task = busy.clone();
            let error_task = error.clone();
            spawn_local(async move {
                let mut videos: Vec<Video> = Vec::new();
                let mut failures = 0usize;
                for source in list.iter() {
                    let resolved = api::resolve_channel(&key, &source.id).await;
                    let channel = match resolved {
                        Ok(channel) => channel,
                        Err(failure) => {
                            ctx_task.api_failed(&failure);
                            failures += 1;
                            continue;
                        }
                    };
                    match api::fetch_uploads(&key, &channel, per_channel).await {
                        Ok(mut uploads) => {
                            for video in uploads.drain(..) {
                                if !videos.iter().any(|existing| existing.id == video.id) {
                                    videos.push(video);
                                }
                            }
                        }
                        Err(failure) => {
                            ctx_task.api_failed(&failure);
                            failures += 1;
                        }
                    }
                }
                busy_task.set(false);
                if videos.is_empty() {
                    error_task.set(Some(if failures > 0 {
                        "No se pudo consultar ningún canal. Revisa la clave o la conexión.".into()
                    } else {
                        "Los canales no tienen subidas públicas.".into()
                    }));
                    return;
                }
                videos.sort_by(|a, b| b.published_ms().total_cmp(&a.published_ms()));
                data_task.set(Some(TimeData {
                    videos,
                    analyzed_at: format::now_ms(),
                    source_count: list.len(),
                }));
                if failures > 0 {
                    ctx_task.toast(format!(
                        "Análisis parcial: fallaron {} de {} canales",
                        failures,
                        list.len()
                    ));
                }
            });
        })
    };

    let head = view_head(
        "times-title",
        "Horarios reales",
        "Cuándo publican los canales que sigues y qué franjas rinden por encima de su mediana.",
        Html::default(),
    );

    if ctx.state.settings.api_key.is_empty() {
        return html! {
            <>
                { head }
                { key_gate(
                    &ctx,
                    "Conecta tu API key",
                    "Los horarios se calculan con fechas y vistas reales de las subidas. Sin clave no hay franjas que medir.",
                ) }
            </>
        };
    }

    let chips = sources.iter().map(|source| {
        let remove = remove_source.clone();
        let id = source.id.clone();
        html! {
            <span class="chip">
                <img src={api::safe_image(&source.avatar)} alt="" />
                <span class="chip__name">{ &source.title }</span>
                <button
                    class="icon-btn"
                    type="button"
                    aria-label={format!("Quitar «{}»", source.title)}
                    onclick={Callback::from(move |_: MouseEvent| remove.emit(id.clone()))}
                >
                    { icon("x", "") }
                </button>
            </span>
        }
    });

    let source_panel = html! {
        <div class="panel">
            <div class="panel-head">
                <div>
                    <h2>{ "Canales a analizar" }</h2>
                    <p class="panel-sub">{ "Se combinan sus subidas para buscar patrones de día y hora." }</p>
                </div>
                <span class="updated">{ format!("{}/{} canales", sources.len(), MAX_SOURCES) }</span>
            </div>
            <div class="chip-row">{ for chips }</div>
            <form class="inline-form" onsubmit={add_source}>
                <input
                    class="input"
                    type="text"
                    value={(*input).clone()}
                    oninput={on_input}
                    placeholder="@canal o youtube.com/@canal"
                    autocomplete="off"
                    spellcheck="false"
                    disabled={sources.len() >= MAX_SOURCES}
                />
                <button
                    class="btn btn--secondary"
                    type="submit"
                    disabled={*adding || *busy || sources.len() >= MAX_SOURCES}
                >
                    { icon("plus", "") }
                    { if *adding { "Añadiendo…" } else { "Añadir" } }
                </button>
            </form>
            <div class="times-run">
                <div class="field">
                    <label>{ "Subidas por canal" }</label>
                    <div class="segmented">
                        <label class="segmented__item">
                            <input
                                type="radio"
                                name="times-limit"
                                value="25"
                                checked={*limit == 25}
                                onchange={{
                                    let limit = limit.clone();
                                    Callback::from(move |_: Event| limit.set(25))
                                }}
                            />
                            <span>{ "25" }</span>
                        </label>
                        <label class="segmented__item">
                            <input
                                type="radio"
                                name="times-limit"
                                value="50"
                                checked={*limit == 50}
                                onchange={{
                                    let limit = limit.clone();
                                    Callback::from(move |_: Event| limit.set(50))
                                }}
                            />
                            <span>{ "50" }</span>
                        </label>
                    </div>
                </div>
                <button class="btn btn--primary" type="button" onclick={analyze} disabled={*busy || sources.is_empty()}>
                    { icon("clock", "") }
                    { if *busy { "Analizando…" } else { "Analizar horarios" } }
                </button>
                <span class="cost-note">
                    { "Cada canal cuesta unas 3 unidades más una por cada 50 subidas." }
                </span>
            </div>
        </div>
    };

    let body = if *busy && data.is_none() {
        html! { <LoadingPanel label="Leyendo las fechas de publicación…" /> }
    } else if let Some(message) = (*error).clone() {
        html! {
            <StatePanel
                title="No se pudieron leer los horarios"
                body={message}
                error=true
                icon_name="alert"
            />
        }
    } else if let Some(current) = (*data).clone() {
        time_results(&current)
    } else {
        html! {
            <StatePanel
                title="Aún no hay franjas"
                body="Añade hasta tres canales y pulsa «Analizar horarios». Se leerán las fechas de sus últimas subidas y las vistas que tienen ahora."
                icon_name="calendar"
            />
        }
    };

    html! {
        <>
            { head }
            { source_panel }
            { body }
        </>
    }
}

fn time_results(data: &TimeData) -> Html {
    let videos = &data.videos;
    let cells = build_cells(videos);
    let overall = analysis::median(
        &videos.iter().map(|video| video.views as f64).collect::<Vec<_>>(),
    )
    .unwrap_or(0.0);
    let analyzed = videos.len();
    let first = videos.iter().map(Video::published_ms).fold(f64::INFINITY, f64::min);
    let last = videos
        .iter()
        .map(Video::published_ms)
        .fold(f64::NEG_INFINITY, f64::max);

    let mut ranked: Vec<(usize, &Cell)> = cells
        .iter()
        .enumerate()
        .filter(|(_, cell)| cell.count >= 2 && cell.index > 0.0)
        .collect();
    ranked.sort_by(|a, b| b.1.index.total_cmp(&a.1.index));
    let top = &ranked[..ranked.len().min(3)];

    let best = top.first().map(|(slot, _)| {
        format!("{} · {} h", format::fmt_weekday(*slot / 4), BLOCKS[*slot % 4])
    });
    let best_delta = top.first().map(|(_, cell)| {
        format!(
            "{}× sobre la mediana · al menos dos subidas",
            format::fmt_float(cell.index, 1)
        )
    });

    let stats = html! {
        <StatStrip items={vec![
            StatItem {
                label: "Subidas analizadas".into(),
                value: format::fmt_int(analyzed as u64),
                delta: Some(format!(
                    "{} · {}",
                    format::plural(data.source_count as u64, "canal", "canales"),
                    format::time_ago(data.analyzed_at)
                )),
            },
            StatItem {
                label: "Última subida".into(),
                value: if last.is_finite() {
                    format::fmt_date(last)
                } else {
                    "—".into()
                },
                delta: Some(if first.is_finite() && last.is_finite() {
                    format!("Desde {}", format::fmt_date(first))
                } else {
                    String::new()
                }),
            },
            StatItem {
                label: "Vistas medianas".into(),
                value: format::fmt_compact(overall.round() as u64),
                delta: Some("Referencia para comparar franjas".into()),
            },
            StatItem {
                label: "Mejor franja".into(),
                value: best.unwrap_or_else(|| "Sin muestra".into()),
                delta: Some(best_delta.unwrap_or_else(|| "Al menos dos subidas".into())),
            },
        ]} />
    };

    let head_row = html! {
        <tr>
            <th scope="col"><span class="sr-only">{ "Día" }</span></th>
            { for BLOCKS.iter().map(|block| html! {
                <th scope="col">{ format!("{block} h") }</th>
            }) }
        </tr>
    };

    let rows = (0..7).map(|weekday| {
        let day = format::fmt_weekday(weekday);
        let day_cells = (0..4).map(|block| {
            let cell = &cells[weekday * 4 + block];
            if cell.count == 0 {
                html! {
                    <td class="heat-cell" data-level="0" title="Sin subidas en esta franja">
                        <span class="v">{ "—" }</span>
                    </td>
                }
            } else {
                html! {
                    <td
                        class="heat-cell"
                        data-level={cell.level.to_string()}
                        title={format!(
                            "{} {} · {} subidas · {} % sobre la mediana",
                            day,
                            BLOCKS[block],
                            cell.count,
                            format::fmt_float(cell.index * 100.0, 0)
                        )}
                    >
                        <span class="v">{ format!("{}×", format::fmt_float(cell.index, 1)) }</span>
                        <span class="n">{ cell.count }</span>
                    </td>
                }
            }
        });
        html! {
            <tr>
                <th scope="row">{ day }</th>
                { for day_cells }
            </tr>
        }
    });

    let slots = if top.is_empty() {
        html! {
            <p class="empty-inline">
                { "Ninguna franja tiene dos subidas todavía. Sube el número de subidas por canal o añade otro canal." }
            </p>
        }
    } else {
        html! {
            <div class="slot-list">
                { for top.iter().map(|(slot, cell)| html! {
                    <div class="slot">
                        <div>
                            <div class="slot__when">
                                { format!("{} · {} h", format::fmt_weekday(*slot / 4), BLOCKS[*slot % 4]) }
                            </div>
                            <div class="slot__meta">
                                { format!(
                                    "{} · {} % sobre la mediana",
                                    format::plural(cell.count as u64, "subida", "subidas"),
                                    format::fmt_float(cell.index * 100.0, 0)
                                ) }
                            </div>
                        </div>
                        <span class="slot__value">{ format::fmt_float(cell.index, 2) }{ "×" }</span>
                    </div>
                }) }
            </div>
        }
    };

    html! {
        <div class="results">
            <div class="panel">
                <div class="panel-head">
                    <div>
                        <h2>{ "Mapa de franjas" }</h2>
                        <p class="panel-sub">
                            { "Día y bloque de publicación. El número grande es la mediana de vistas de esa franja dividida entre la mediana general; la cifra pequeña son las subidas." }
                        </p>
                    </div>
                    <span class="updated">{ format!("Analizado {}", format::time_ago(data.analyzed_at)) }</span>
                </div>
                { stats }
                <div class="heatmap-wrap">
                    <table class="heatmap">
                        <thead>{ head_row }</thead>
                        <tbody>{ for rows }</tbody>
                    </table>
                </div>
                <div class="heat-legend">
                    <span>{ "Menos" }</span>
                    <span data-level="1"></span>
                    <span data-level="2"></span>
                    <span data-level="3"></span>
                    <span data-level="4"></span>
                    <span data-level="5"></span>
                    <span>{ "Más" }</span>
                </div>
            </div>

            <div class="panel-split">
                <div class="panel">
                    <div class="panel-head">
                        <div>
                            <h2>{ "Mejores franjas" }</h2>
                            <p class="panel-sub">{ "Solo franjas con dos o más subidas." }</p>
                        </div>
                    </div>
                    { slots }
                </div>
                <div class="panel">
                    <div class="panel-head">
                        <div>
                            <h2>{ "Cómo leerlo" }</h2>
                            <p class="panel-sub">{ "Las vistas son las de hoy, no las del día de publicación." }</p>
                        </div>
                    </div>
                    <div class="insights">
                        <p class="insight">
                            { "Una franja con índice " }
                            <strong>{ "1,5×" }</strong>
                            { " rinde un 50 % más que la mediana de los vídeos analizados. No prueba causalidad: mezcla día, hora, tema y antigüedad." }
                        </p>
                        <p class="insight">
                            { "Los vídeos más antiguos acumulan más vistas. Para reducir ese sesgo, compara franjas con subidas recientes y un número parecido de vídeos." }
                        </p>
                        <p class="insight">
                            { "La hora se muestra en la zona horaria de este navegador. YouTube publica las fechas en UTC y aquí se convierten a tu hora local." }
                        </p>
                    </div>
                    <Method summary="Método y límites">
                        <p>
                            { "Se piden las últimas 25 o 50 subidas de cada canal, se agrupan por día de la semana y bloque de seis horas, y se calcula la mediana de vistas de cada grupo. El índice es esa mediana dividida entre la mediana de todas las subidas analizadas." }
                        </p>
                        <p>
                            { "Los niveles de color van de 1 a 5 según el índice. Las franjas vacías quedan sin color. No se usan datos de audiencia del canal: solo fechas y vistas públicas." }
                        </p>
                    </Method>
                </div>
            </div>
        </div>
    }
}
