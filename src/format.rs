use js_sys::Date;
use wasm_bindgen::JsValue;

const MONTHS: [&str; 12] = [
    "ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sep", "oct", "nov", "dic",
];

const WEEKDAYS: [&str; 7] = [
    "lunes",
    "martes",
    "miércoles",
    "jueves",
    "viernes",
    "sábado",
    "domingo",
];

pub fn now_ms() -> f64 {
    Date::now()
}

pub fn iso_ms(value: &str) -> f64 {
    if value.is_empty() {
        return f64::NAN;
    }
    Date::parse(value)
}

fn date(ms: f64) -> Date {
    Date::new(&JsValue::from_f64(ms))
}

pub fn fmt_int(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            out.push('.');
        }
        out.push(ch);
    }
    out
}

pub fn fmt_signed(value: i64) -> String {
    if value < 0 {
        format!("-{}", fmt_int(value.unsigned_abs()))
    } else {
        fmt_int(value as u64)
    }
}

pub fn fmt_float(value: f64, digits: usize) -> String {
    if !value.is_finite() {
        return "-".into();
    }
    format!("{:.*}", digits, value).replace('.', ",")
}

pub fn fmt_compact(value: u64) -> String {
    if value < 1_000 {
        return fmt_int(value);
    }
    if value < 10_000 {
        let scaled = value as f64 / 1_000.0;
        return format!("{} mil", trim_decimal(&fmt_float(scaled, 1)));
    }
    if value < 1_000_000 {
        return format!("{} mil", fmt_int(value / 1_000));
    }
    if value < 1_000_000_000 {
        let scaled = value as f64 / 1_000_000.0;
        return format!("{} M", trim_decimal(&fmt_float(scaled, 1)));
    }
    let scaled = value as f64 / 1_000_000_000.0;
    format!("{} mil M", trim_decimal(&fmt_float(scaled, 1)))
}

fn trim_decimal(value: &str) -> String {
    value
        .strip_suffix(",0")
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string())
}

pub fn fmt_percent(fraction: f64, digits: usize) -> String {
    if !fraction.is_finite() {
        return "-".into();
    }
    format!("{} %", fmt_float(fraction * 100.0, digits))
}

pub fn plural(count: u64, singular: &str, plural: &str) -> String {
    format!("{} {}", fmt_int(count), if count == 1 { singular } else { plural })
}

pub fn fmt_duration(total_seconds: u64) -> String {
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

pub fn iso_duration_secs(value: &str) -> Option<u64> {
    let mut chars = value.chars().peekable();
    if chars.next()? != 'P' {
        return None;
    }
    let mut total = 0f64;
    let mut number = String::new();
    let mut in_time = false;
    for ch in chars {
        match ch {
            'T' => {
                in_time = true;
                number.clear();
            }
            '0'..='9' | '.' => number.push(ch),
            'D' => {
                total += number.parse::<f64>().ok()? * 86_400.0;
                number.clear();
            }
            'H' if in_time => {
                total += number.parse::<f64>().ok()? * 3_600.0;
                number.clear();
            }
            'M' if in_time => {
                total += number.parse::<f64>().ok()? * 60.0;
                number.clear();
            }
            'S' if in_time => {
                total += number.parse::<f64>().ok()?;
                number.clear();
            }
            _ => return None,
        }
    }
    Some(total.round() as u64)
}

pub fn fmt_date(ms: f64) -> String {
    if !ms.is_finite() {
        return "-".into();
    }
    let d = date(ms);
    format!(
        "{} {} {}",
        d.get_date(),
        MONTHS[d.get_month() as usize % 12],
        d.get_full_year()
    )
}

pub fn fmt_day_month(ms: f64) -> String {
    if !ms.is_finite() {
        return "-".into();
    }
    let d = date(ms);
    format!("{} {}", d.get_date(), MONTHS[d.get_month() as usize % 12])
}

pub fn fmt_weekday_date(ms: f64) -> String {
    if !ms.is_finite() {
        return "-".into();
    }
    let d = date(ms);
    format!(
        "{} {} {}",
        &WEEKDAYS[weekday_index(ms)],
        d.get_date(),
        MONTHS[d.get_month() as usize % 12]
    )
}

pub fn fmt_weekday(day_index: usize) -> &'static str {
    WEEKDAYS[day_index % 7]
}

pub fn fmt_time(ms: f64) -> String {
    if !ms.is_finite() {
        return "-".into();
    }
    let d = date(ms);
    format!("{:02}:{:02}", d.get_hours(), d.get_minutes())
}

pub fn fmt_datetime(ms: f64) -> String {
    if !ms.is_finite() {
        return "-".into();
    }
    format!("{} · {}", fmt_weekday_date(ms), fmt_time(ms))
}

pub fn fmt_datetime_local_value(ms: f64) -> String {
    if !ms.is_finite() {
        return String::new();
    }
    let d = date(ms);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}",
        d.get_full_year(),
        d.get_month() + 1,
        d.get_date(),
        d.get_hours(),
        d.get_minutes()
    )
}

pub fn parse_datetime_local(value: &str) -> Option<f64> {
    let (date_part, time_part) = value.split_once('T')?;
    let mut date_bits = date_part.split('-');
    let year: u32 = date_bits.next()?.parse().ok()?;
    let month: u32 = date_bits.next()?.parse().ok()?;
    let day: u32 = date_bits.next()?.parse().ok()?;
    if date_bits.next().is_some() {
        return None;
    }
    let mut time_bits = time_part.split(':');
    let hours: u32 = time_bits.next()?.parse().ok()?;
    let minutes: u32 = time_bits.next()?.parse().ok()?;
    if time_bits.next().is_some() {
        return None;
    }
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hours > 23 || minutes > 59 {
        return None;
    }
    let parsed = Date::new_with_year_month_day_hr_min(year, month as i32 - 1, day as i32, hours as i32, minutes as i32);
    let ms = parsed.get_time();
    ms.is_finite().then_some(ms)
}

pub fn local_date_key(ms: f64) -> String {
    let d = date(ms);
    format!(
        "{:04}-{:02}-{:02}",
        d.get_full_year(),
        d.get_month() + 1,
        d.get_date()
    )
}

pub fn next_hour(ms: f64) -> f64 {
    let d = date(ms);
    Date::new_with_year_month_day_hr_min(
        d.get_full_year(),
        d.get_month() as i32,
        d.get_date() as i32,
        d.get_hours() as i32 + 1,
        0,
    )
    .get_time()
}

pub fn weekday_index(ms: f64) -> usize {
    // JS 0 is Sunday; the heatmap reads Monday first.
    ((date(ms).get_day() + 6) % 7) as usize
}

pub fn hour_of_day(ms: f64) -> u32 {
    date(ms).get_hours()
}

pub fn iso_from_now(days_ago: u32) -> String {
    let target = Date::new(&JsValue::from_f64(now_ms() - f64::from(days_ago) * 86_400_000.0));
    String::from(target.to_iso_string())
}

pub fn time_ago(ms: f64) -> String {
    time_ago_at(ms, now_ms())
}

pub fn time_ago_at(ms: f64, now: f64) -> String {
    if !ms.is_finite() {
        return String::new();
    }
    let seconds = (now - ms) / 1000.0;
    let future = seconds < 0.0;
    let abs = seconds.abs();
    let label = if abs < 45.0 {
        "unos segundos".to_string()
    } else if abs < 3_600.0 {
        format!("{} min", (abs / 60.0).round() as u64)
    } else if abs < 86_400.0 {
        format!("{} h", (abs / 3_600.0).round() as u64)
    } else if abs < 604_800.0 {
        format!("{} d", (abs / 86_400.0).round() as u64)
    } else if abs < 2_592_000.0 {
        format!("{} sem", (abs / 604_800.0).round() as u64)
    } else if abs < 31_536_000.0 {
        format!("{} meses", (abs / 2_592_000.0).round() as u64)
    } else {
        format!("{} años", (abs / 31_536_000.0).round() as u64)
    };
    if future {
        format!("en {label}")
    } else {
        format!("hace {label}")
    }
}

pub fn new_id() -> String {
    let random = js_sys::Math::random();
    format!("{:.0}-{:06}", Date::now(), (random * 1_000_000.0) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ints_use_spanish_grouping() {
        assert_eq!(fmt_int(0), "0");
        assert_eq!(fmt_int(999), "999");
        assert_eq!(fmt_int(1_234), "1.234");
        assert_eq!(fmt_int(1_234_567), "1.234.567");
    }

    #[test]
    fn compact_numbers_stay_readable() {
        assert_eq!(fmt_compact(950), "950");
        assert_eq!(fmt_compact(1_500), "1,5 mil");
        assert_eq!(fmt_compact(850_000), "850 mil");
        assert_eq!(fmt_compact(2_300_000), "2,3 M");
    }

    #[test]
    fn durations_parse_and_format() {
        assert_eq!(iso_duration_secs("PT1H2M3S"), Some(3723));
        assert_eq!(iso_duration_secs("PT45S"), Some(45));
        assert_eq!(iso_duration_secs("P1DT2H"), Some(93_600));
        assert_eq!(iso_duration_secs("P0D"), Some(0));
        assert_eq!(iso_duration_secs("nope"), None);
        assert_eq!(fmt_duration(3723), "1:02:03");
        assert_eq!(fmt_duration(75), "1:15");
    }

    #[test]
    fn percents_and_plurals() {
        assert_eq!(fmt_percent(0.0243, 2), "2,43 %");
        assert_eq!(plural(1, "vídeo", "vídeos"), "1 vídeo");
        assert_eq!(plural(3, "vídeo", "vídeos"), "3 vídeos");
    }

    #[test]
    fn relative_time_reads_naturally() {
        let now = 1_000_000_000_000.0;
        assert_eq!(time_ago_at(now - 30_000.0, now), "hace unos segundos");
        assert_eq!(time_ago_at(now - 7_200_000.0, now), "hace 2 h");
        assert_eq!(time_ago_at(now + 7_200_000.0, now), "en 2 h");
    }
}
