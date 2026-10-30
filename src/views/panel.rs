use gloo_timers::callback::Interval;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::analysis;
use crate::api;
use crate::app::{key_gate, AppCtx, ConfirmKind};
use crate::components::{
    icon, LineChart, LoadingPanel, StatItem, StatStrip, StatePanel,
};
use crate::format;
use crate::models::{Channel, PanelCache, Snapshot, Video};
use crate::storage;
use crate::views::{engagement_text, signed, view_head, video_cell};

#[derive(Clone, PartialEq)]
struct PanelData {
    channel: Channel,
    videos: Vec<Video>,
    fetched_at: f64,
    snapshots: Vec<Snapshot>,
    cached: bool,
}

fn canonical_ref(reference: &str) -> String {
    let mut value = reference.trim().to_lowercase();
    for prefix in ["https://", "http://"] {
        value = value.strip_prefix(prefix).unwrap_or(&value).to_string();
    }
    for prefix in ["www.", "m."] {
        value = value.strip_prefix(prefix).unwrap_or(&value).to_string();
    }
    value = value.trim_end_matches('/').to_string();
    for suffix in ["/videos", "/shorts", "/streams", "/featured", "/about", "/playlists"] {
        if let Some(stripped) = value.strip_suffix(suffix) {
            value = stripped.to_string();
        }
    }
    value
}

fn matches_ref(channel: &Channel, reference: &str) -> bool {
    let input = canonical_ref(reference);
    if input.is_empty() {
        return false;
    }
    let handle = channel.handle.to_lowercase();
    let bare = handle.trim_start_matches('@');
    let id = channel.id.to_lowercase();
    input == handle
        || input == bare
        || input == format!("@{bare}")
        || input == id
        || input == channel.title.to_lowercase()
        || input.ends_with(&format!("/{bare}"))
        || input.ends_with(&format!("/@{bare}"))
        || input.ends_with(&format!("/{id}"))
}

fn delta_line(snapshots: &[Snapshot], pick: fn(&Snapshot) -> u64) -> Option<String> {
    if snapshots.len() < 2 {
        return None;
    }
    let last = snapshots.last()?;
    let previous = snapshots.get(snapshots.len() - 2)?;
    let diff = pick(last) as i64 - pick(previous) as i64;
    let when = format::time_ago(previous.at);
    Some(if diff == 0 {
        format!("Sin cambios · {when}")
    } else {
        format!("{} · {when}", signed(diff))
    })
}

#[function_component(PanelView)]
pub fn panel_view() -> Html {
    let ctx = use_context::<AppCtx>().expect("app context");
    let input = use_state(String::new);
    let data = use_state(|| None::<PanelData>);
    let error = use_state(|| None::<String>);
    let busy = use_state(|| false);
    let reload = use_state(|| 0u32);
    let in_flight = use_mut_ref(|| false);

    let channel_ref = ctx.state.settings.channel_ref.clone();
    let api_key = ctx.state.settings.api_key.clone();
    let refresh_secs = ctx.state.settings.refresh_secs;

    {
        let data = data.clone();
        let error = error.clone();
        use_effect_with(channel_ref.clone(), move |reference| {
            if reference.is_empty() {
                data.set(None);
                error.set(None);
                return;
            }
            let mismatched = (*data)
                .as_ref()
                .map(|existing| !matches_ref(&existing.channel, reference))
                .unwrap_or(false);
            if mismatched {
                data.set(None);
                error.set(None);
            }
            if data.is_none() {
                if let Some(cache) = storage::load_panel_cache() {
                    if matches_ref(&cache.channel, reference) {
                        let snapshots = storage::load_snapshots()
                            .get(&cache.channel.id)
                            .cloned()
                            .unwrap_or_default();
                        data.set(Some(PanelData {
                            channel: cache.channel,
                            videos: cache.videos,
                            fetched_at: cache.fetched_at,
                            snapshots,
                            cached: true,
                        }));
                    }
                }
            }
        });
    }

    {
        let ctx = ctx.clone();
        let data = data.clone();
        let error = error.clone();
        let busy = busy.clone();
        let in_flight = in_flight.clone();
        use_effect_with(
            (channel_ref.clone(), api_key.clone(), *reload),
            move |(reference, key, _)| {
                let reference = reference.clone();
                let key = key.clone();
                if reference.is_empty() || key.is_empty() || *in_flight.borrow() {
                    return;
                }
                *in_flight.borrow_mut() = true;
                busy.set(true);
                spawn_local(async move {
                    let result = api::fetch_panel(&key, &reference).await;
                    *in_flight.borrow_mut() = false;
                    busy.set(false);
                    match result {
                        Ok(payload) => {
                            let fetched_at = format::now_ms();
                            let snapshots = storage::record_snapshot(
                                &payload.channel.id,
                                payload.channel.subs,
                                payload.channel.views,
                                payload.channel.video_count,
                            );
                            storage::save_panel_cache(&PanelCache {
                                channel: payload.channel.clone(),
                                videos: payload.videos.clone(),
                                fetched_at,
                            });
                            error.set(None);
                            data.set(Some(PanelData {
                                channel: payload.channel,
                                videos: payload.videos,
                                fetched_at,
                                snapshots,
                                cached: false,
                            }));
                        }
                        Err(failure) => {
                            ctx.api_failed(&failure);
                            error.set(Some(failure.message));
                        }
                    }
                });
            },
        );
    }

    {
        let reload = reload.clone();
        use_effect_with(refresh_secs, move |secs| {
            let cleanup: Box<dyn FnOnce()> = if *secs == 0 {
                Box::new(|| {})
            } else {
                let handle = reload.clone();
                let interval = Interval::new(*secs * 1_000, move || handle.set(*handle + 1));
                Box::new(move || drop(interval))
            };
            cleanup
        });
    }

    let on_input = {
        let input = input.clone();
        Callback::from(move |event: InputEvent| {
            let element = event.target_unchecked_into::<web_sys::HtmlInputElement>();
            input.set(element.value());
        })
    };

    let on_connect = {
        let ctx = ctx.clone();
        let input = input.clone();
        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();
            let reference = input.trim().to_string();
            if reference.is_empty() {
                ctx.toast("Escribe el @handle o la URL del canal");
                return;
            }
            ctx.set_channel_ref(reference);
            input.set(String::new());
        })
    };

    let refresh = {
        let reload = reload.clone();
        Callback::from(move |_: MouseEvent| reload.set(*reload + 1))
    };

    let disconnect = {
        let ctx = ctx.clone();
        Callback::from(move |_: MouseEvent| ctx.open_confirm(ConfirmKind::DisconnectChannel))
    };

    let retry = refresh.clone();

    let connected = !channel_ref.is_empty();
    let actions = if connected {
        html! {
            <div class="channel-actions">
                <button class="btn btn--secondary btn--sm" type="button" onclick={refresh.clone()} disabled={*busy}>
                    { icon("refresh", "") }
                    { if *busy { "Actualizando…" } else { "Actualizar" } }
                </button>
                <button class="icon-btn" type="button" aria-label="Desconectar el canal" onclick={disconnect}>
                    { icon("x", "") }
                </button>
            </div>
        }
    } else {
        Html::default()
    };

    let head = view_head(
        "panel-title",
        "Panel del canal",
        "Suscriptores, vistas y actividad reales del canal, sin cifras de ejemplo.",
        actions,
    );

    if api_key.is_empty() {
        return html! {
            <>
                { head }
                { key_gate(
                    &ctx,
                    "Conecta tu API key",
                    "El panel consulta YouTube Data API v3 directamente desde este navegador. Añade tu clave para ver datos reales del canal.",
                ) }
            </>
        };
    }

    if !connected {
        return html! {
            <>
                { head }
                <div class="panel">
                    <div class="setup-grid">
                        <div>
                            <h2 class="setup-title">{ "Conecta tu canal" }</h2>
                            <p class="setup-copy">
                                { "Pega la URL del canal o su @handle. Verás suscriptores, vistas y tus últimos vídeos reales, además de una serie diaria para medir el crecimiento." }
                            </p>
                            <form class="connect-form" onsubmit={on_connect}>
                                <div class="connect-row">
                                    <div class="field">
                                        <label for="channel-ref">{ "Canal de YouTube" }</label>
                                        <input
                                            id="channel-ref"
                                            class="input"
                                            type="text"
                                            value={(*input).clone()}
                                            oninput={on_input}
                                            placeholder="@tucanal o youtube.com/@tucanal"
                                            autocomplete="off"
                                            spellcheck="false"
                                        />
                                    </div>
                                    <button class="btn btn--primary" type="submit">{ "Conectar" }</button>
                                </div>
                            </form>
                            <p class="cost-note">
                                { "La primera consulta cuesta unas 3 unidades de cuota. Los datos del canal son públicos." }
                            </p>
                        </div>
                        <aside class="setup-aside">
                            <h3>{ "Qué se guarda" }</h3>
                            <p>
                                { "El canal conectado, una medición diaria de suscriptores y vistas, y los últimos 12 vídeos. Todo queda en este navegador, nunca en un servidor." }
                            </p>
                            <p class="hint">
                                { "Sin datos de ejemplo ni plantillas inventadas: cada cifra viene de la API de YouTube." }
                            </p>
                        </aside>
                    </div>
                </div>
            </>
        };
    }

    let current = (*data)
        .clone()
        .filter(|existing| matches_ref(&existing.channel, &channel_ref));

    let Some(data) = current else {
        if let Some(message) = (*error).clone() {
            return html! {
                <>
                    { head }
                    <StatePanel
                        title="No se pudo cargar el canal"
                        body={message}
                        error=true
                        icon_name="alert"
                        action_label="Reintentar"
                        action={retry}
                    />
                </>
            };
        }
        return html! {
            <>
                { head }
                <LoadingPanel label="Cargando el canal…" />
            </>
        };
    };

    let PanelData {
        channel,
        videos,
        fetched_at,
        snapshots,
        cached,
    } = &data;

    let subs_value = if channel.hidden_subs {
        "Ocultas".to_string()
    } else {
        format::fmt_int(channel.subs)
    };
    let views_value = format::fmt_compact(channel.views);
    let avg_views = analysis::avg_views(videos);
    let engagement = analysis::median_engagement(videos);
    let cadence = analysis::uploads_per_week(
        &videos.iter().map(Video::published_ms).collect::<Vec<_>>(),
    );
    let video_views: Vec<f64> = videos.iter().map(|video| video.views as f64).collect();
    let median_views = analysis::median(&video_views).unwrap_or(0.0);
    let low_threshold = (median_views / 2.0).round() as u64;
    let low_share = analysis::low_view_share(videos, low_threshold);
    let best = videos.iter().max_by_key(|video| video.views);

    let sub_delta = delta_line(snapshots, |snapshot| snapshot.subs);
    let view_delta = delta_line(snapshots, |snapshot| snapshot.views);

    let stats = html! {
        <StatStrip items={vec![
            StatItem {
                label: "Suscriptores".into(),
                value: subs_value,
                delta: sub_delta.clone(),
            },
            StatItem {
                label: "Vistas totales".into(),
                value: views_value,
                delta: view_delta.clone(),
            },
            StatItem {
                label: "Vistas medias (12)".into(),
                value: avg_views
                    .map(|value| format::fmt_compact(value.round() as u64))
                    .unwrap_or_else(|| "—".into()),
                delta: None,
            },
            StatItem {
                label: "Interacción mediana".into(),
                value: engagement
                    .map(|value| format::fmt_percent(value, 2))
                    .unwrap_or_else(|| "—".into()),
                delta: None,
            },
        ]} />
    };

    let subs_points: Vec<(f64, f64)> = snapshots
        .iter()
        .map(|snapshot| (snapshot.at, snapshot.subs as f64))
        .collect();
    let views_points: Vec<(f64, f64)> = snapshots
        .iter()
        .map(|snapshot| (snapshot.at, snapshot.views as f64))
        .collect();

    let mut insights = Vec::new();
    if let Some(best) = best {
        insights.push(html! {
            <p class="insight">
                { "Tu vídeo más visto de los últimos 12 es " }
                <strong>{ format!("«{}»", best.title) }</strong>
                { " con " }
                <strong>{ format::fmt_compact(best.views) }</strong>
                { " vistas." }
            </p>
        });
    }
    if let Some(engagement) = engagement {
        insights.push(html! {
            <p class="insight">
                { "La interacción mediana es " }
                <strong>{ format::fmt_percent(engagement, 2) }</strong>
                { " (likes y comentarios por vista). Un valor bajo indica que el vídeo se ve pero no se comenta." }
            </p>
        });
    }
    if videos.len() >= 2 {
        insights.push(html! {
            <p class="insight">
                { "Publicas " }
                <strong>{ format!("{} vídeos por semana", format::fmt_float(cadence, 1)) }</strong>
                { " según las fechas de tus últimas subidas." }
            </p>
        });
        if median_views > 0.0 {
            insights.push(html! {
                <p class="insight">
                    { "El " }
                    <strong>{ format::fmt_percent(low_share, 0) }</strong>
                    { " de esos vídeos queda por debajo de la mitad de tu mediana (" }
                    <strong>{ format::fmt_compact(low_threshold) }</strong>
                    { " vistas)." }
                </p>
            });
        }
    }
    if snapshots.len() >= 2 {
        if let Some(delta) = &sub_delta {
            insights.push(html! {
                <p class="insight">
                    { "Suscriptores desde la medición anterior: " }
                    <strong>{ delta.clone() }</strong>
                    { "." }
                </p>
            });
        }
    }
    if insights.is_empty() {
        insights.push(html! {
            <p class="insight">
                { "Conecta el canal y vuelve tras el siguiente vídeo para ver patrones de crecimiento." }
            </p>
        });
    }

    let updated = format!(
        "Actualizado {}{}",
        format::time_ago(*fetched_at),
        if *cached { " · copia guardada" } else { "" }
    );

    let banner = (*error).clone().map(|message| html! {
        <div class="notice" role="status">
            <div>
                <strong>{ "No se pudo actualizar." }</strong>
                <span>{ message }</span>
            </div>
            <button class="btn btn--secondary btn--sm" type="button" onclick={retry.clone()}>
                { "Reintentar" }
            </button>
        </div>
    });

    let chart_panel = |title: &str, subtitle: &str, points: Vec<(f64, f64)>, empty: &str| {
        let body = if points.len() >= 2 {
            html! { <div class="chart"><LineChart points={points} label={title.to_string()} /></div> }
        } else {
            html! {
                <div class="chart-empty">
                    <strong>{ "Aún no hay serie" }</strong>
                    <span>{ empty.to_string() }</span>
                </div>
            }
        };
        html! {
            <div>
                <div class="panel-head">
                    <div>
                        <h3>{ title.to_string() }</h3>
                        <p class="panel-sub">{ subtitle.to_string() }</p>
                    </div>
                </div>
                { body }
            </div>
        }
    };

    let video_rows = videos.iter().map(|video| {
        let meta = format!(
            "{} · {}",
            format::fmt_date(video.published_ms()),
            video
                .duration
                .map(format::fmt_duration)
                .unwrap_or_else(|| "—".into())
        );
        html! {
            <tr>
                <td>{ video_cell(video, meta) }</td>
                <td class="num" title={format::fmt_int(video.views)}>{ format::fmt_compact(video.views) }</td>
                <td class="num">{ engagement_text(video) }</td>
                <td class="center">
                    <a
                        class="table-link"
                        href={api::video_url(&video.id)}
                        target="_blank"
                        rel="noopener noreferrer"
                        aria-label={format!("Abrir «{}» en YouTube", video.title)}
                    >
                        { icon("external", "") }
                    </a>
                </td>
            </tr>
        }
    });

    let channel_meta = if channel.hidden_subs {
        format!(
            "{} · suscriptores ocultos · {}",
            channel.handle,
            format::plural(channel.video_count, "vídeo", "vídeos")
        )
    } else {
        format!(
            "{} · {} suscriptores · {}",
            channel.handle,
            format::fmt_compact(channel.subs),
            format::plural(channel.video_count, "vídeo", "vídeos")
        )
    };

    html! {
        <>
            { head }
            { banner.unwrap_or_default() }
            <div class="panel">
                <div class="channel-head">
                    <img class="channel-avatar" src={api::safe_image(&channel.avatar)} alt="" />
                    <div class="channel-id">
                        <div class="channel-name">{ &channel.title }</div>
                        <div class="channel-meta">{ channel_meta }</div>
                    </div>
                    <a
                        class="text-link"
                        href={api::channel_url(&channel.id)}
                        target="_blank"
                        rel="noopener noreferrer"
                    >
                        { "Ver en YouTube" }
                        { icon("external", "") }
                    </a>
                    <span class="updated">{ updated }</span>
                </div>
                { stats }
            </div>

            <div class="panel-split">
                <div class="panel">
                    <div class="panel-head">
                        <div>
                            <h2>{ "Serie diaria" }</h2>
                            <p class="panel-sub">{ "Una medición por día, guardada solo en este navegador." }</p>
                        </div>
                    </div>
                    <div class="stack">
                        { if !channel.hidden_subs {
                            chart_panel(
                                "Suscriptores",
                                "Últimas 180 mediciones diarias.",
                                subs_points,
                                "Cada visita guarda una medición del día. Con dos días distintos verás aquí la línea real de suscriptores.",
                            )
                        } else {
                            Html::default()
                        } }
                        { chart_panel(
                            "Vistas acumuladas",
                            "Suma total del canal en cada medición.",
                            views_points,
                            "La serie empieza con la primera visita. Vuelve mañana para ver la pendiente real de vistas.",
                        ) }
                    </div>
                </div>

                <div class="panel">
                    <div class="panel-head">
                        <div>
                            <h2>{ "Lectura del canal" }</h2>
                            <p class="panel-sub">{ "Calculado con tus últimos 12 vídeos, no con estimaciones." }</p>
                        </div>
                    </div>
                    <div class="insights">
                        { for insights }
                    </div>
                </div>
            </div>

            <div class="panel">
                <div class="panel-head">
                    <div>
                        <h2>{ "Últimos 12 vídeos" }</h2>
                        <p class="panel-sub">{ "Ordenados por fecha de publicación." }</p>
                    </div>
                </div>
                if videos.is_empty() {
                    <div class="empty-inline">
                        <p>{ "Este canal no expone vídeos públicos o no tiene subidas." }</p>
                    </div>
                } else {
                    <div class="table-wrap">
                        <table class="data-table">
                            <thead>
                                <tr>
                                    <th scope="col">{ "Vídeo" }</th>
                                    <th scope="col" class="num">{ "Vistas" }</th>
                                    <th scope="col" class="num">{ "Interacción" }</th>
                                    <th scope="col"><span class="sr-only">{ "Abrir" }</span></th>
                                </tr>
                            </thead>
                            <tbody>
                                { for video_rows }
                            </tbody>
                        </table>
                    </div>
                }
            </div>
        </>
    }
}
