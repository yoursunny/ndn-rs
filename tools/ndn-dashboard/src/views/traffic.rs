use std::collections::VecDeque;
use dioxus::prelude::*;
use crate::app::AppCtx;
use crate::types::ThroughputSample;

#[component]
pub fn Traffic() -> Element {
    let ctx = use_context::<AppCtx>();
    let counters = ctx.counters.read();
    let measurements = ctx.measurements.read();
    let throughput = ctx.throughput.read();

    rsx! {
        // ── Throughput sparkline ──────────────────────────────────────────
        {render_throughput(&throughput)}

        // ── Per-face counters ─────────────────────────────────────────────
        div { class: "section",
            div { class: "section-title", "Face Traffic Counters" }
            if counters.is_empty() {
                div { class: "empty", "No counter data." }
            } else {
                table {
                    thead {
                        tr {
                            th { "Face" }
                            th { "In ↓ Interests" }
                            th { "In ↓ Data" }
                            th { "Out ↑ Interests" }
                            th { "Out ↑ Data" }
                            th { "In ↓ Bytes" }
                            th { "Out ↑ Bytes" }
                        }
                    }
                    tbody {
                        for c in counters.iter() {
                            tr {
                                td { class: "mono", "{c.face_id}" }
                                td { class: "mono", "{c.in_interests}" }
                                td { class: "mono", "{c.in_data}" }
                                td { class: "mono", "{c.out_interests}" }
                                td { class: "mono", "{c.out_data}" }
                                td { class: "mono", "{fmt_bytes(c.in_bytes)}" }
                                td { class: "mono", "{fmt_bytes(c.out_bytes)}" }
                            }
                        }
                    }
                }
            }
        }

        // ── Measurements ──────────────────────────────────────────────────
        div { class: "section",
            div { class: "section-title", "Prefix Measurements" }
            if measurements.is_empty() {
                div { class: "empty", "No measurements yet. Accumulates as Interests are forwarded." }
            } else {
                table {
                    thead {
                        tr {
                            th { "Prefix" }
                            th { "Sat. Rate" }
                            th { "RTT per Face" }
                        }
                    }
                    tbody {
                        for m in measurements.iter() {
                            tr {
                                td { class: "mono", "{m.prefix}" }
                                td {
                                    span { class: "{m.sat_rate_class()}",
                                        "{(m.satisfaction_rate * 100.0):.1}%"
                                    }
                                }
                                td { class: "mono",
                                    if m.face_rtts.is_empty() {
                                        "—"
                                    } else {
                                        {
                                            let s = m.face_rtts.iter()
                                                .map(|r| format!("face{}={:.1}ms", r.face_id, r.srtt_ms))
                                                .collect::<Vec<_>>().join("  |  ");
                                            rsx! { "{s}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn render_throughput(hist: &VecDeque<ThroughputSample>) -> Element {
    let last = hist.back();
    let title = match last {
        Some(s) => format!(
            "Throughput  —  {} /s ↓ in  /  {} /s ↑ out",
            fmt_bytes(s.in_bytes), fmt_bytes(s.out_bytes)
        ),
        None => "Throughput — collecting data…".to_string(),
    };

    // Compute max for scaling
    let max_bytes = hist.iter()
        .map(|s| s.in_bytes.max(s.out_bytes))
        .max()
        .unwrap_or(1)
        .max(1);

    rsx! {
        div { class: "section",
            div { class: "section-title", "{title}" }
            if hist.len() < 2 {
                div { class: "empty", "Collecting throughput data… (updates every 3 s)" }
            } else {
                div {
                    style: "display:flex;align-items:flex-end;gap:2px;height:80px;padding:4px 0;",
                    for sample in hist.iter() {
                        {
                            let in_h  = (sample.in_bytes  as f64 / max_bytes as f64 * 72.0) as u32;
                            let out_h = (sample.out_bytes as f64 / max_bytes as f64 * 72.0) as u32;
                            rsx! {
                                div { style: "display:flex;gap:1px;align-items:flex-end;",
                                    div { style: "width:4px;height:{in_h}px;background:#58a6ff;border-radius:1px 1px 0 0;" }
                                    div { style: "width:4px;height:{out_h}px;background:#3fb950;border-radius:1px 1px 0 0;" }
                                }
                            }
                        }
                    }
                }
                div { style: "font-size:11px;color:#8b949e;margin-top:4px;",
                    span { style: "color:#58a6ff;", "■" }
                    " in (bytes/s)  "
                    span { style: "color:#3fb950;", "■" }
                    " out (bytes/s)  — last 60 samples × 3s = 3 min window"
                }
            }
        }
    }
}

fn fmt_bytes(b: u64) -> String {
    if b >= 1_073_741_824 { format!("{:.1} GB", b as f64 / 1_073_741_824.0) }
    else if b >= 1_048_576 { format!("{:.1} MB", b as f64 / 1_048_576.0) }
    else if b >= 1024 { format!("{:.1} KB", b as f64 / 1024.0) }
    else { format!("{b} B") }
}
