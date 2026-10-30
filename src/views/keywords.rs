use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::analysis;
use crate::api;
use crate::app::{key_gate, AppCtx};
use crate::components::{
    icon, Gauge, LoadingPanel, Method, StatItem, StatStrip, StatePanel,
};
use crate::format;
use crate::models::{KeywordAnalysis, SavedKeyword};
use crate::storage;
use crate::views::{engagement_text, external_link, view_head, video_cell};

const COST_UNITS: u32 = 101;
const LOW_VIEW_THRESHOLD: u64 = 100_000;
const DEMAND_REFERENCE: f64 = 1_000_000.0;
const COMPETITION_REFERENCE: f64 = 5_000_000.0;

fn build_analysis(payload: api::KeywordPayload, region: &str) -> KeywordAnalysis {
    let median_views = analysis::median(
        &payload
            .videos
            .iter()
            .map(|video| video.views as f64)
            .collect::<Vec<_>>(),
    )
    .unwrap_or(0.0);
    let top3_views = analysis::top_average(&payload.videos, 3);
    let low_share = analysis::low_view_share(&payload.videos, LOW_VIEW_THRESHOLD);
    let demand = analysis::log_scale(median_views, DEMAND_REFERENCE);
    let competition = analysis::log_scale(top3_views, COMPETITION_REFERENCE);
    let score = (100.0 * (0.5 * demand + 0.3 * (1.0 - competition) + 0.2 * low_share))
        .round()
        .clamp(0.0, 100.0) as u32;
    let titles: Vec<String> = payload
        .videos
        .iter()
        .map(|video| video.title.clone())
        .collect();
    let terms = analysis::extract_terms(&titles, 12);
    let hashtags = analysis::extract_hashtags(&titles, 8);
    KeywordAnalysis {
        query: payload.query,
        region: region.to_string(),
        fetched_at: format::now_ms(),
        total_results: payload.total_results,
        videos: payload.videos,
        median_views,
        top3_views,
        low_share,
        demand,
        competition,
        score,
        terms,
        hashtags,
    }
}

fn score_label(score: u32) -> &'static str {
    if score >= 75 {
        "Oportunidad alta"
    } else if score >= 50 {
        "Oportunidad media"
    } else {
        "Muy competida"
    }
}

#[function_component(KeywordsView)]
pub fn keywords_view() -> Html {
    let ctx = use_context::<AppCtx>().expect("app context");
    let query = use_state(String::new);
    let data = use_state(|| None::<KeywordAnalysis>);
    let saved = use_state(storage::load_keywords);
    let error = use_state(|| None::<String>);
    let busy = use_state(|| false);

    let on_query = {
        let query = query.clone();
        Callback::from(move |event: InputEvent| {
            let element = event.target_unchecked_into::<web_sys::HtmlInputElement>();
            query.set(element.value());
        })
    };

    let run_search = {
        let ctx = ctx.clone();
        let query = query.clone();
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
                ctx.toast("Escribe una keyword para analizar");
                return;
            }
            let region = ctx.state.settings.region.clone();
            busy.set(true);
            error.set(None);
            let ctx_task = ctx.clone();
            let data_task = data.clone();
            let error_task = error.clone();
            let busy_task = busy.clone();
            spawn_local(async move {
                let result = api::fetch_keyword(&key, &term, &region).await;
                busy_task.set(false);
                match result {
                    Ok(payload) => {
                        data_task.set(Some(build_analysis(payload, &region)));
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

    let save = {
        let ctx = ctx.clone();
        let data = data.clone();
        let saved = saved.clone();
        Callback::from(move |_: MouseEvent| {
            let Some(analysis) = (*data).clone() else {
                return;
            };
            let mut entries = (*saved).clone();
            if entries
                .iter()
                .any(|entry| entry.analysis.query.eq_ignore_ascii_case(&analysis.query))
            {
                ctx.toast("Ese análisis ya está guardado");
                return;
            }
            entries.insert(
                0,
                SavedKeyword {
                    id: format::new_id(),
                    saved_at: format::now_ms(),
                    analysis,
                },
            );
            storage::save_keywords(&entries);
            saved.set(entries);
            ctx.toast("Análisis guardado");
        })
    };

    let head = view_head(
        "keywords-title",
        "Investigación de keywords",
        "Demanda, competencia y huecos reales a partir de los resultados de búsqueda de YouTube.",
        Html::default(),
    );

    if ctx.state.settings.api_key.is_empty() {
        return html! {
            <>
                { head }
                { key_gate(
                    &ctx,
                    "Conecta tu API key",
                    "La investigación de keywords consulta resultados reales. Sin clave no hay demanda ni competencia que medir.",
                ) }
            </>
        };
    }

    let form = html! {
        <div class="panel">
            <div class="panel-head">
                <div>
                    <h2>{ "Analizar una keyword" }</h2>
                    <p class="panel-sub">{ "Busca la keyword y se miden los 25 primeros resultados." }</p>
                </div>
                <span class="updated">{ format!("Región {}", ctx.state.settings.region) }</span>
            </div>
            <form class="toolbar" onsubmit={on_submit}>
                <div class="field field--grow">
                    <label for="keyword-query">{ "Keyword" }</label>
                    <input
                        id="keyword-query"
                        class="input"
                        type="text"
                        value={(*query).clone()}
                        oninput={on_query}
                        placeholder="por ejemplo, recetas rápidas"
                        autocomplete="off"
                        spellcheck="false"
                    />
                </div>
                <div class="field field--submit">
                    <button class="btn btn--primary" type="submit" disabled={*busy}>
                        { icon("search", "") }
                        { if *busy { "Analizando…" } else { "Analizar" } }
                    </button>
                </div>
            </form>
            <p class="cost-note">
                { format!("Cada análisis cuesta unas {COST_UNITS} unidades. Se guarda la foto de los resultados, no las vistas en vivo.") }
            </p>
        </div>
    };

    let saved_panel = if saved.is_empty() {
        Html::default()
    } else {
        let entries = (*saved).clone();
        html! {
            <div class="panel">
                <div class="panel-head">
                    <div>
                        <h2>{ "Análisis guardados" }</h2>
                        <p class="panel-sub">{ "Pasos de 25 resultados ya consultados, sin gastar cuota." }</p>
                    </div>
                </div>
                <div class="chip-row">
                    { for entries.into_iter().map(|entry| {
                        let open = {
                            let data = data.clone();
                            let query = query.clone();
                            let ctx = ctx.clone();
                            let analysis = entry.analysis.clone();
                            Callback::from(move |_: MouseEvent| {
                                query.set(analysis.query.clone());
                                data.set(Some(analysis.clone()));
                                ctx.toast("Análisis cargado sin gastar cuota");
                            })
                        };
                        let remove = {
                            let saved = saved.clone();
                            let ctx = ctx.clone();
                            let id = entry.id.clone();
                            Callback::from(move |_: MouseEvent| {
                                let mut entries = (*saved).clone();
                                entries.retain(|item| item.id != id);
                                storage::save_keywords(&entries);
                                saved.set(entries);
                                ctx.toast("Análisis eliminado");
                            })
                        };
                        html! {
                            <span class="chip">
                                <span class="chip__name">{ &entry.analysis.query }</span>
                                <span class="badge">{ entry.analysis.score }</span>
                                <button class="icon-btn" type="button" aria-label={format!("Abrir «{}»", entry.analysis.query)} onclick={open}>
                                    { icon("search", "") }
                                </button>
                                <button class="icon-btn" type="button" aria-label={format!("Eliminar «{}»", entry.analysis.query)} onclick={remove}>
                                    { icon("trash", "") }
                                </button>
                            </span>
                        }
                    }) }
                </div>
            </div>
        }
    };

    let results = if *busy && data.is_none() {
        html! { <LoadingPanel label="Analizando la keyword…" /> }
    } else if let Some(message) = (*error).clone() {
        html! {
            <StatePanel
                title="El análisis no se completó"
                body={message}
                error=true
                icon_name="alert"
                action_label="Reintentar"
                action={retry.clone()}
            />
        }
    } else if let Some(current) = (*data).clone() {
        keyword_results(&current, save)
    } else {
        html! {
            <StatePanel
                title="Sin keyword analizada"
                body="Escribe una keyword para ver cuánta demanda hay, qué resultados la dominan y cuántos huecos de baja competencia existen."
                icon_name="tag"
            />
        }
    };

    html! {
        <>
            { head }
            { form }
            { saved_panel }
            { results }
        </>
    }
}

fn keyword_results(analysis_data: &KeywordAnalysis, save: Callback<MouseEvent>) -> Html {
    let analysis = analysis_data;
    let terms_html = if analysis.terms.is_empty() {
        html! { <p class="empty-inline">{ "Sin términos repetidos en los títulos de los resultados." }</p> }
    } else {
        html! {
            <div class="chip-row">
                { for analysis.terms.iter().map(|term| html! {
                    <span class="chip">
                        <span class="chip__name">{ &term.term }</span>
                        <span class="badge">{ term.count }</span>
                    </span>
                }) }
            </div>
        }
    };
    let hashtags_html = if analysis.hashtags.is_empty() {
        html! { <p class="empty-inline">{ "Ningún título visible incluye hashtags." }</p> }
    } else {
        html! {
            <div class="chip-row">
                { for analysis.hashtags.iter().map(|tag| html! {
                    <span class="tag">{ &tag.term }</span>
                }) }
            </div>
        }
    };

    let low_count = analysis
        .videos
        .iter()
        .filter(|video| video.views < LOW_VIEW_THRESHOLD)
        .count();

    let score_note = format!(
        "Demanda {:.2} · competencia {:.2} · hueco de baja competencia {:.0} %.",
        analysis.demand,
        analysis.competition,
        analysis.low_share * 100.0
    );

    let stats = html! {
        <StatStrip
            compact=true
            items={vec![
                StatItem {
                    label: "Vistas medianas".into(),
                    value: format::fmt_compact(analysis.median_views.round() as u64),
                    delta: Some("De los 25 primeros resultados".into()),
                },
                StatItem {
                    label: "Media del top 3".into(),
                    value: format::fmt_compact(analysis.top3_views.round() as u64),
                    delta: Some("Mide la fuerza de los líderes".into()),
                },
                StatItem {
                    label: "Resultados en total".into(),
                    value: format::fmt_int(analysis.total_results),
                    delta: Some("Estimación de YouTube para esa keyword".into()),
                },
            ]}
        />
    };

    let rows = analysis.videos.iter().map(|video| {
        let meta = format!(
            "{} · {}",
            video.channel_title,
            format::fmt_date(video.published_ms())
        );
        let signal = if video.views < LOW_VIEW_THRESHOLD {
            html! { <span class="badge badge--red">{ "Baja competencia" }</span> }
        } else {
            html! { <span class="badge">{ "—" }</span> }
        };
        html! {
            <tr>
                <td>{ video_cell(video, meta) }</td>
                <td class="num" title={format::fmt_int(video.views)}>{ format::fmt_compact(video.views) }</td>
                <td class="num">{ engagement_text(video) }</td>
                <td>{ signal }</td>
                <td class="center">{ external_link(&video.id, &video.title) }</td>
            </tr>
        }
    });

    html! {
        <div class="results">
            <div class="panel">
                <div class="panel-head">
                    <div>
                        <h2>{ format!("«{}»", analysis.query) }</h2>
                        <p class="panel-sub">
                            { format!(
                                "Región {} · consultado {}",
                                analysis.region,
                                format::time_ago(analysis.fetched_at)
                            ) }
                        </p>
                    </div>
                    <button class="btn btn--secondary btn--sm" type="button" onclick={save}>
                        { icon("plus", "") }
                        { "Guardar análisis" }
                    </button>
                </div>
                <div class="score-head">
                    <Gauge value={analysis.score} />
                    <div class="score-copy">
                        <div class="score-label">{ score_label(analysis.score) }</div>
                        <p class="score-note">
                            { score_note }
                            { " Se pondera la demanda, la fuerza del top 3 y la proporción de resultados con menos de 100.000 vistas." }
                        </p>
                    </div>
                </div>
                { stats }
                <div class="insights">
                    <p class="insight">
                        { "El resultado mediano tiene " }
                        <strong>{ format!("{} vistas", format::fmt_compact(analysis.median_views.round() as u64)) }</strong>
                        { " y el top 3 promedia " }
                        <strong>{ format!("{} vistas", format::fmt_compact(analysis.top3_views.round() as u64)) }</strong>
                        { "." }
                    </p>
                    <p class="insight">
                        { format!("{low_count} de los {} resultados quedan por debajo de 100.000 vistas: ", analysis.videos.len()) }
                        <strong>{ format::fmt_percent(analysis.low_share, 0) }</strong>
                        { " del total. Cuanto más alto, más fácil entrar en la primera página." }
                    </p>
                    <p class="insight">
                        { "YouTube estima " }
                        <strong>{ format::plural(analysis.total_results, "resultado", "resultados") }</strong>
                        { " para esta búsqueda. La cifra es una estimación del propio buscador." }
                    </p>
                </div>
            </div>

            <div class="panel">
                <div class="panel-head">
                    <div>
                        <h2>{ "Primeros 25 resultados" }</h2>
                        <p class="panel-sub">{ "Orden de relevancia de YouTube, con vistas e interacción reales." }</p>
                    </div>
                </div>
                if analysis.videos.is_empty() {
                    <div class="empty-inline">
                        <p>{ "No hay resultados públicos para esa keyword en esta región." }</p>
                    </div>
                } else {
                    <div class="table-wrap">
                        <table class="data-table">
                            <thead>
                                <tr>
                                    <th scope="col">{ "Vídeo" }</th>
                                    <th scope="col" class="num">{ "Vistas" }</th>
                                    <th scope="col" class="num">{ "Interacción" }</th>
                                    <th scope="col">{ "Señal" }</th>
                                    <th scope="col"><span class="sr-only">{ "Abrir" }</span></th>
                                </tr>
                            </thead>
                            <tbody>
                                { for rows }
                            </tbody>
                        </table>
                    </div>
                }
            </div>

            <div class="panel">
                <div class="panel-head">
                    <div>
                        <h2>{ "Patrones en los títulos" }</h2>
                        <p class="panel-sub">{ "Términos y hashtags que aparecen en los resultados reales." }</p>
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
                <Method summary="Cómo se calcula la oportunidad">
                    <p>
                        { "Demanda = escala logarítmica de las vistas medianas, con 1.000.000 como referencia. Competencia = escala logarítmica de la media del top 3, con 5.000.000 como referencia. Hueco = proporción de resultados con menos de 100.000 vistas." }
                    </p>
                    <p>
                        { "La puntuación es 100 × (0,50 × demanda + 0,30 × (1 − competencia) + 0,20 × hueco). Es una fórmula transparente sobre datos de la API, no una predicción de YouTube." }
                    </p>
                    <p>
                        { "Una búsqueda cuesta unas 101 unidades de cuota y devuelve, como máximo, 25 vídeos. «Resultados en total» es la estimación de pageInfo, que YouTube redondea." }
                    </p>
                </Method>
            </div>
        </div>
    }
}
