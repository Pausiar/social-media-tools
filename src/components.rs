use yew::prelude::*;

pub fn icon(name: &str, class: &str) -> Html {
    let class_name = if class.is_empty() {
        "icon".to_string()
    } else {
        format!("icon {class}")
    };
    html! {
        <svg class={class_name} aria-hidden="true"><use href={format!("#i-{name}")}></use></svg>
    }
}

#[derive(Properties, PartialEq)]
pub struct StatePanelProps {
    pub title: AttrValue,
    pub body: AttrValue,
    #[prop_or_default]
    pub error: bool,
    #[prop_or_default]
    pub icon_name: Option<AttrValue>,
    #[prop_or_default]
    pub action_label: Option<AttrValue>,
    #[prop_or_default]
    pub action: Option<Callback<MouseEvent>>,
}

#[function_component(StatePanel)]
pub fn state_panel(props: &StatePanelProps) -> Html {
    let action = match (&props.action_label, &props.action) {
        (Some(label), Some(callback)) => Some(html! {
            <button class="btn btn--secondary" type="button" onclick={callback.clone()}>
                { label.clone() }
            </button>
        }),
        _ => None,
    };
    html! {
        <div class={classes!("panel", "state", props.error.then_some("state--error"))}>
            if let Some(name) = &props.icon_name {
                <span class="state-icon">{ icon(name, "") }</span>
            }
            <h2>{ props.title.clone() }</h2>
            <p>{ props.body.clone() }</p>
            { action }
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub struct LoadingPanelProps {
    #[prop_or_else(|| AttrValue::from("Cargando datos reales…"))]
    pub label: AttrValue,
}

#[function_component(LoadingPanel)]
pub fn loading_panel(props: &LoadingPanelProps) -> Html {
    html! {
        <div class="panel" aria-busy="true">
            <div class="panel-head">
                <div>
                    <h2>{ props.label.clone() }</h2>
                    <p class="panel-sub">{ "Consultando la API de YouTube." }</p>
                </div>
            </div>
            <div class="skeleton-stack">
                <div class="skeleton skeleton-block"></div>
                <div class="skeleton-stack">
                    { for (0..3).map(|_| html! {
                        <div class="skeleton-row">
                            <div class="skeleton skeleton--thumb"></div>
                            <div class="skeleton-lines">
                                <div class="skeleton skeleton--line"></div>
                                <div class="skeleton skeleton--line skeleton--short"></div>
                            </div>
                        </div>
                    }) }
                </div>
            </div>
        </div>
    }
}

#[derive(Clone, PartialEq)]
pub struct StatItem {
    pub label: String,
    pub value: String,
    pub delta: Option<String>,
}

#[derive(Properties, PartialEq)]
pub struct StatStripProps {
    pub items: Vec<StatItem>,
    #[prop_or_default]
    pub compact: bool,
}

#[function_component(StatStrip)]
pub fn stat_strip(props: &StatStripProps) -> Html {
    html! {
        <div class={classes!("stat-strip", props.compact.then_some("stat-strip--3"))}>
            { for props.items.iter().map(|item| html! {
                <div class="stat">
                    <span class="stat__label">{ &item.label }</span>
                    <span class="stat__value">{ &item.value }</span>
                    if let Some(delta) = &item.delta {
                        <span class={classes!("stat__delta", delta.starts_with('+').then_some("is-up"))}>
                            { delta }
                        </span>
                    }
                </div>
            }) }
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub struct GaugeProps {
    pub value: u32,
}

#[function_component(Gauge)]
pub fn gauge(props: &GaugeProps) -> Html {
    let circumference = 2.0 * std::f64::consts::PI * 36.0;
    let value = props.value.min(100) as f64;
    let offset = circumference * (1.0 - value / 100.0);
    html! {
        <svg class="gauge" viewBox="0 0 88 88" role="img" aria-label={format!("Puntuación {}", props.value)}>
            <circle class="gauge__track" cx="44" cy="44" r="36"></circle>
            <circle
                class="gauge__value"
                cx="44"
                cy="44"
                r="36"
                stroke-dasharray={format!("{circumference:.2}")}
                stroke-dashoffset={format!("{offset:.2}")}
            ></circle>
            <text class="gauge__text" x="44" y="52" text-anchor="middle">{ props.value }</text>
        </svg>
    }
}

#[derive(Properties, PartialEq)]
pub struct LineChartProps {
    pub points: Vec<(f64, f64)>,
    #[prop_or_default]
    pub label: AttrValue,
}

#[function_component(LineChart)]
pub fn line_chart(props: &LineChartProps) -> Html {
    if props.points.len() < 2 {
        return Html::default();
    }
    let points = &props.points;
    let (min, max) = points.iter().fold(
        (f64::INFINITY, f64::NEG_INFINITY),
        |(min, max), (_, value)| (min.min(*value), max.max(*value)),
    );
    let span = (max - min).max(1.0);
    let count = points.len() as f64;
    let plot_left = 48.0;
    let plot_right = 12.0;
    let plot_top = 12.0;
    let plot_bottom = 24.0;
    let width = 600.0;
    let height = 170.0;
    let x_for = |index: usize| {
        plot_left + (width - plot_left - plot_right) * index as f64 / (count - 1.0)
    };
    let y_for = |value: f64| {
        plot_top + (height - plot_top - plot_bottom) * (1.0 - (value - min) / span)
    };
    let line = points
        .iter()
        .enumerate()
        .map(|(index, (_, value))| format!("{:.1},{:.1}", x_for(index), y_for(*value)))
        .collect::<Vec<_>>()
        .join(" ");
    let first_x = x_for(0);
    let last_x = x_for(points.len() - 1);
    let base_y = height - plot_bottom;
    let area = format!(
        "M{first_x:.1},{base_y:.1} L{line} L{last_x:.1},{base_y:.1} Z"
    );
    let last = points.last().map(|(_, value)| *value).unwrap_or_default();
    let first = points.first().map(|(at, _)| *at).unwrap_or_default();
    let last_at = points.last().map(|(at, _)| *at).unwrap_or_default();
    let label = if props.label.is_empty() {
        AttrValue::from("Serie de datos")
    } else {
        props.label.clone()
    };
    let grid = (0..3)
        .map(|index| {
            let value = min + span * index as f64 / 2.0;
            let y = y_for(value);
            html! {
                <g>
                    <line class="chart__grid" x1={plot_left.to_string()} x2={(width - plot_right).to_string()} y1={format!("{y:.1}")} y2={format!("{y:.1}")}></line>
                    <text x="0" y={format!("{:.1}", y + 3.5)}>{ crate::format::fmt_compact(value.max(0.0) as u64) }</text>
                </g>
            }
        })
        .collect::<Html>();
    html! {
        <svg viewBox={format!("0 0 {width:.0} {height:.0}")} role="img" aria-label={label}>
            { grid }
            <path class="chart__area" d={area}></path>
            <polyline class="chart__line" points={line}></polyline>
            <circle class="chart__dot" cx={format!("{last_x:.1}")} cy={format!("{:.1}", y_for(last))} r="3.5"></circle>
            <text x={format!("{first_x:.1}")} y={(height - 4.0).to_string()} text-anchor="start">{ crate::format::fmt_day_month(first) }</text>
            <text x={format!("{last_x:.1}")} y={(height - 4.0).to_string()} text-anchor="end">{ crate::format::fmt_day_month(last_at) }</text>
        </svg>
    }
}

#[derive(Properties, PartialEq)]
pub struct MethodProps {
    pub summary: AttrValue,
    pub children: Children,
}

#[function_component(Method)]
pub fn method(props: &MethodProps) -> Html {
    html! {
        <details class="method">
            <summary>{ props.summary.clone() }</summary>
            { for props.children.iter() }
        </details>
    }
}
