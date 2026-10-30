use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::analysis;
use crate::api;
use crate::app::AppCtx;
use crate::components::{icon, Gauge, Method, StatePanel};
use crate::format;
use crate::models::Video;
use crate::score::{self, Platform};
use crate::storage;
use crate::views::{view_head, video_cell};

fn platform_from(value: &str) -> Platform {
    match value {
        "shorts" => Platform::Shorts,
        "tiktok" => Platform::Tiktok,
        _ => Platform::Youtube,
    }
}

#[function_component(TitlesView)]
pub fn titles_view() -> Html {
    let ctx = use_context::<AppCtx>().expect("app context");
    let title = use_state(|| storage::get("titleDraft", String::new()));
    let keyword = use_state(|| storage::get("titleKeyword", String::new()));
    let platform = use_state(|| {
        platform_from(&storage::get("titlePlatform", String::from("youtube")))
    });
    let benchmark = use_state(|| None::<Vec<Video>>);
    let benchmarking = use_state(|| false);
    let bench_error = use_state(|| None::<String>);

    let on_title = {
        let title = title.clone();
        Callback::from(move |event: InputEvent| {
            let element = event.target_unchecked_into::<web_sys::HtmlTextAreaElement>();
            let value = element.value();
            storage::set("titleDraft", &value);
            title.set(value);
        })
    };

    let on_keyword = {
        let keyword = keyword.clone();
        Callback::from(move |event: InputEvent| {
            let element = event.target_unchecked_into::<web_sys::HtmlInputElement>();
            let value = element.value();
            storage::set("titleKeyword", &value);
            keyword.set(value);
        })
    };

    let on_platform = {
        let platform = platform.clone();
        Callback::from(move |event: Event| {
            let element = event.target_unchecked_into::<web_sys::HtmlInputElement>();
            let value = element.value();
            storage::set("titlePlatform", &value);
            platform.set(platform_from(&value));
        })
    };

    let run_benchmark = {
        let ctx = ctx.clone();
        let keyword = keyword.clone();
        let title = title.clone();
        let benchmark = benchmark.clone();
        let benchmarking = benchmarking.clone();
        let bench_error = bench_error.clone();
        Callback::from(move |_: MouseEvent| {
            if *benchmarking {
                return;
            }
            let key = ctx.state.settings.api_key.clone();
            if key.is_empty() {
                ctx.open_settings();
                return;
            }
            let term = if keyword.trim().is_empty() {
                title.trim().to_string()
            } else {
                keyword.trim().to_string()
            };
            if term.is_empty() {
                ctx.toast("Escribe una keyword o un título para buscar la referencia");
                return;
            }
            let region = ctx.state.settings.region.clone();
            benchmarking.set(true);
            bench_error.set(None);
            let ctx_task = ctx.clone();
            let benchmark_task = benchmark.clone();
            let benchmarking_task = benchmarking.clone();
            let bench_error_task = bench_error.clone();
            spawn_local(async move {
                let result = api::fetch_benchmark(&key, &term, &region).await;
                benchmarking_task.set(false);
                match result {
                    Ok(videos) => benchmark_task.set(Some(videos)),
                    Err(failure) => {
                        ctx_task.api_failed(&failure);
                        bench_error_task.set(Some(failure.message));
                    }
                }
            });
        })
    };

    let report = score::analyze((*title).as_str(), (*keyword).as_str(), *platform);
    let head = view_head(
        "analyzer-title",
        "Analizador de títulos",
        "Seis factores medibles antes de publicar, con el corte real del feed y una referencia opcional de YouTube.",
        Html::default(),
    );

    let char_class = classes!(
        "char-count",
        (report.chars > report.limit).then_some("is-over")
    );
    let (preview_head, preview_rest) = score::preview_parts((*title).as_str(), report.cut);
    let over = report.chars > report.limit;
    let preview_foot = if title.trim().is_empty() {
        "Escribe un título para ver cómo lo cortaría el feed.".to_string()
    } else if over {
        format!(
            "Se recorta: sobran {} caracteres para {}.",
            report.chars - report.limit,
            platform.label()
        )
    } else if report.chars > report.cut {
        format!(
            "El feed cortará hacia el carácter {}. La parte gris queda fuera de la primera línea.",
            report.cut
        )
    } else {
        "Se verá completo en el feed, sin recorte.".to_string()
    };

    let platform_label = match *platform {
        Platform::Youtube => "El título completo se ve en la página del vídeo; el feed corta antes.",
        Platform::Shorts => "En Shorts el título se superpone al vídeo y se corta muy pronto.",
        Platform::Tiktok => "TikTok permite más caracteres, pero el feed muestra menos.",
    };

    let score_note = if title.trim().is_empty() {
        "Sin título no hay puntuación. La escala va de 0 a 100 y suma seis factores.".to_string()
    } else {
        format!(
            "{} caracteres y {} palabras. La escala va de 0 a 100; no predice vistas, mide reglas que puedes corregir ahora.",
            report.chars, report.words
        )
    };

    let factors = report.factors.iter().map(|factor| {
        html! {
            <div class="factor">
                <div class="factor-top">
                    <span class="factor-label">{ factor.label }</span>
                    <span class="factor-value">{ format!("{}/{}", factor.score, factor.max) }</span>
                </div>
                <progress class="factor-bar" value={factor.score.to_string()} max={factor.max.to_string()}></progress>
                <p class="factor-note">{ &factor.note }</p>
            </div>
        }
    });

    let tips = report.tips.iter().map(|tip| {
        html! {
            <div class={classes!("tip", tip.hard.then_some("tip--hard"))}>
                { icon(if tip.hard { "alert" } else { "check" }, "") }
                <span>{ &tip.text }</span>
            </div>
        }
    });

    let benchmark_content = if ctx.state.settings.api_key.is_empty() {
        html! {
            <p class="form-note">
                { "Conecta tu API key para ver los diez primeros resultados reales de esa búsqueda." }
            </p>
        }
    } else {
        html! {
            <>
                <button
                    class="btn btn--secondary"
                    type="button"
                    onclick={run_benchmark}
                    disabled={*benchmarking}
                >
                    { icon("search", "") }
                    { if *benchmarking { "Buscando…" } else { "Ver los 10 primeros de YouTube" } }
                </button>
                <p class="cost-note">
                    { "Una consulta de referencia cuesta unas 101 unidades de cuota. Se busca la keyword, o el título si no hay keyword." }
                </p>
            </>
        }
    };

    let benchmark_result = match (&*benchmark, &*bench_error) {
        (_, Some(message)) => html! {
            <p class="form-note is-error">{ message.clone() }</p>
        },
        (Some(videos), _) if videos.is_empty() => html! {
            <div class="empty-inline"><p>{ "Sin resultados públicos para esa búsqueda." }</p></div>
        },
        (Some(videos), _) => {
            let median = analysis::median(
                &videos.iter().map(|video| video.views as f64).collect::<Vec<_>>(),
            )
            .unwrap_or(0.0);
            let rows = videos.iter().enumerate().map(|(index, video)| {
                let meta = format!(
                    "{} · {}",
                    video.channel_title,
                    format::fmt_date(video.published_ms())
                );
                html! {
                    <tr>
                        <td class="center">{ index + 1 }</td>
                        <td>{ video_cell(video, meta) }</td>
                        <td class="num">{ format::fmt_compact(video.views) }</td>
                    </tr>
                }
            });
            html! {
                <div class="benchmark-result">
                    <p class="benchmark-summary">
                        { "La mediana de estos diez resultados es " }
                        <strong>{ format!("{} vistas", format::fmt_compact(median.round() as u64)) }</strong>
                        { ". Es la vara con la que compite tu título, no una garantía de rendimiento." }
                    </p>
                    <div class="table-wrap">
                        <table class="data-table">
                            <thead>
                                <tr>
                                    <th scope="col" class="center">{ "#" }</th>
                                    <th scope="col">{ "Resultado real" }</th>
                                    <th scope="col" class="num">{ "Vistas" }</th>
                                </tr>
                            </thead>
                            <tbody>{ for rows }</tbody>
                        </table>
                    </div>
                </div>
            }
        }
        _ => Html::default(),
    };

    html! {
        <>
            { head }
            <div class="analyzer-grid">
                <div class="panel">
                    <div class="panel-head">
                        <div>
                            <h2>{ "Tu título" }</h2>
                            <p class="panel-sub">{ "Se analiza mientras escribes y queda guardado en este navegador." }</p>
                        </div>
                    </div>

                    <div class="segmented" role="group" aria-label="Plataforma">
                        <label class="segmented__item">
                            <input
                                type="radio"
                                name="title-platform"
                                value="youtube"
                                checked={*platform == Platform::Youtube}
                                onchange={on_platform.clone()}
                            />
                            <span>{ "YouTube" }</span>
                        </label>
                        <label class="segmented__item">
                            <input
                                type="radio"
                                name="title-platform"
                                value="shorts"
                                checked={*platform == Platform::Shorts}
                                onchange={on_platform.clone()}
                            />
                            <span>{ "Shorts" }</span>
                        </label>
                        <label class="segmented__item">
                            <input
                                type="radio"
                                name="title-platform"
                                value="tiktok"
                                checked={*platform == Platform::Tiktok}
                                onchange={on_platform.clone()}
                            />
                            <span>{ "TikTok" }</span>
                        </label>
                    </div>

                    <div class="field">
                        <label for="title-input">
                            { "Título" }
                            <span class="label-hint">{ format!(" · máximo {}", report.limit) }</span>
                        </label>
                        <textarea
                            id="title-input"
                            class="input"
                            rows="3"
                            value={(*title).clone()}
                            oninput={on_title}
                            placeholder="El título que estás pensando publicar"
                            spellcheck="false"
                        ></textarea>
                    </div>

                    <div class="field field--spaced">
                        <label for="title-keyword">
                            { "Keyword objetivo " }
                            <span class="label-hint">{ "opcional" }</span>
                        </label>
                        <input
                            id="title-keyword"
                            class="input"
                            type="text"
                            value={(*keyword).clone()}
                            oninput={on_keyword}
                            placeholder="la búsqueda por la que quieres aparecer"
                            autocomplete="off"
                            spellcheck="false"
                        />
                    </div>

                    <div class="preview-block">
                        <div class="preview-head">
                            <span>{ "Vista previa del feed" }</span>
                            <span class={char_class}>
                                { format!("{} / {}", report.chars, report.limit) }
                            </span>
                        </div>
                        <div class="preview">
                            if title.trim().is_empty() {
                                <p class="preview__empty">{ "El corte se calcula con la plataforma elegida." }</p>
                            } else {
                                <div class="preview__title">
                                    { preview_head }
                                    <span class="preview__rest">{ preview_rest }</span>
                                </div>
                            }
                        </div>
                        <p class="preview__foot">{ preview_foot }</p>
                        if !title.trim().is_empty() && report.chars > report.cut {
                            <p class="preview__foot">{ platform_label }</p>
                        }
                    </div>

                    <Method summary="Cómo se calcula la puntuación">
                        <p>
                            { "Longitud (20), gancho (20), claridad (20), curiosidad (20), legibilidad (10) y señales de spam (10). El total se limita a 100." }
                        </p>
                        <p>
                            { "El gancho premia cifras, promesas concretas y contrastes. La claridad premia que la keyword aparezca y que el título se entienda. La legibilidad mide la media de caracteres por palabra y evita mayúsculas o símbolos. El corte del feed es aproximado: YouTube trunca por píxeles, no por caracteres exactos." }
                        </p>
                    </Method>
                </div>

                <div class="panel">
                    <div class="panel-head">
                        <div>
                            <h2>{ "Lectura del título" }</h2>
                            <p class="panel-sub">{ "Todo se calcula en este navegador, sin enviar el título a ningún sitio." }</p>
                        </div>
                    </div>

                    <div class="score-head">
                        <Gauge value={report.total} />
                        <div class="score-copy">
                            <div class="score-label">{ report.verdict }</div>
                            <p class="score-note">{ score_note }</p>
                        </div>
                    </div>

                    <div>{ for factors }</div>

                    <div class="tips">{ for tips }</div>

                    <div class="benchmark-block">
                        <div class="panel-head">
                            <div>
                                <h3>{ "Referencia real de YouTube" }</h3>
                                <p class="panel-sub">{ "Los diez primeros resultados para tu keyword o título." }</p>
                            </div>
                        </div>
                        { benchmark_content }
                        { benchmark_result }
                    </div>
                </div>
            </div>

            if report.total == 0 {
                <StatePanel
                    title="Empieza por el título"
                    body="El analizador funciona sin API key. Escribe una propuesta y verás la puntuación, los factores y el punto exacto en que el feed la corta."
                    icon_name="type"
                />
            }
        </>
    }
}
