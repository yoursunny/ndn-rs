use dioxus::prelude::*;

use crate::app::{AppCtx, ConnState, DashCmd, RouterCmd, ROUTER_LOG, ROUTER_RUNNING};

#[component]
pub fn Overview() -> Element {
    let ctx = use_context::<AppCtx>();
    let status = ctx.status.read();
    let faces  = ctx.faces.read();
    let keys   = ctx.security_keys.read();

    // Security health — first identity key.
    let (sec_name, sec_badge_class, sec_badge_label, sec_has_cert) =
        if let Some(k) = keys.first() {
            let (cls, lbl) = k.expiry_badge();
            (k.name.clone(), cls, lbl, k.has_cert)
        } else {
            ("(none)".to_string(), "badge badge-gray", "—".to_string(), false)
        };

    let mut edu_dismissed = use_signal(|| false);

    rsx! {
        // ── Education snippet (B7): Zero trust by default ────────────────────
        if !*edu_dismissed.read() {
            div { class: "edu-card",
                div { style: "display:flex;gap:12px;align-items:flex-start;",
                    div { style: "flex-shrink:0;width:60px;text-align:center;padding-top:4px;",
                        div { class: "drop-packet", "Interest /ndn/…" }
                        div { style: "font-size:9px;color:#f85149;margin-top:4px;", "✕ unsigned" }
                    }
                    div { style: "flex:1;",
                        div { style: "display:flex;justify-content:space-between;align-items:flex-start;",
                            div { style: "font-size:13px;font-weight:600;color:#58a6ff;margin-bottom:4px;",
                                "Zero Trust by Default"
                            }
                            button {
                                style: "background:none;border:none;color:#8b949e;cursor:pointer;font-size:13px;padding:0;",
                                onclick: move |_| edu_dismissed.set(true),
                                "✕"
                            }
                        }
                        div { style: "font-size:12px;color:#8b949e;line-height:1.6;",
                            "Every NDN packet carries a "
                            strong { style: "color:#c9d1d9;", "cryptographic signature." }
                            " A packet with a broken or missing signature chain is dropped — not forwarded. "
                            "Configure your identity and trust anchor in the "
                            span { style: "color:#58a6ff;", "Security" }
                            " tab."
                        }
                    }
                }
            }
        }

        // ── Stat cards ──────────────────────────────────────────────────────
        div { class: "cards",
            StatCard { label: "Faces",  value: status.as_ref().map(|s| s.n_faces).unwrap_or(0) }
            StatCard { label: "FIB",    value: status.as_ref().map(|s| s.n_fib).unwrap_or(0) }
            StatCard { label: "PIT",    value: status.as_ref().map(|s| s.n_pit).unwrap_or(0) }
            StatCard { label: "CS",     value: status.as_ref().map(|s| s.n_cs).unwrap_or(0) }
        }

        // ── Security health ─────────────────────────────────────────────────
        div { class: "section",
            div { class: "section-title",
                span {
                    "data-tooltip": "NDN uses cryptographic identities to sign every packet.\nEach router has an Ed25519 key pair stored in the PIB.\nThis card shows your primary identity and certificate status.",
                    "Security Health"
                }
            }
            div { style: "display:flex;align-items:center;gap:16px;flex-wrap:wrap;",
                div {
                    div { style: "font-size:11px;color:#8b949e;margin-bottom:4px;text-transform:uppercase;letter-spacing:.5px;",
                        "Identity"
                    }
                    div { class: "mono", style: "font-size:13px;color:#c9d1d9;", "{sec_name}" }
                }
                div {
                    div { style: "font-size:11px;color:#8b949e;margin-bottom:4px;text-transform:uppercase;letter-spacing:.5px;",
                        "Certificate"
                    }
                    if sec_has_cert {
                        span { class: "badge badge-green", "issued" }
                    } else {
                        span {
                            class: "badge badge-yellow",
                            "data-tooltip": "No certificate — this key cannot be used for signed Interests or Data until a CA issues a certificate via NDNCERT.",
                            "no cert"
                        }
                    }
                }
                div {
                    div { style: "font-size:11px;color:#8b949e;margin-bottom:4px;text-transform:uppercase;letter-spacing:.5px;",
                        "Expiry"
                    }
                    span { class: "{sec_badge_class}", "{sec_badge_label}" }
                }
                if keys.len() > 1 {
                    {
                        let n = keys.len();
                        rsx! {
                            div { style: "margin-left:auto;",
                                span { class: "badge badge-blue", "{n} keys total" }
                            }
                        }
                    }
                }
            }
            if !sec_has_cert && sec_name != "(none)" {
                div { style: "margin-top:12px;padding-top:12px;border-top:1px solid #21262d;font-size:12px;color:#8b949e;",
                    "Use the "
                    span { style: "color:#58a6ff;cursor:pointer;", "Security" }
                    " tab to enroll with a CA and obtain a certificate for this identity."
                }
            }
        }

        // ── Active faces ────────────────────────────────────────────────────
        div { class: "section",
            div { class: "section-title", "Active Faces" }
            if faces.is_empty() {
                div { class: "empty", "No faces — connect to a router to see data." }
            } else {
                table {
                    thead {
                        tr {
                            th { "ID" }
                            th { "Kind" }
                            th { "Remote URI" }
                            th { "Local URI" }
                            th { "Persistency" }
                        }
                    }
                    tbody {
                        for face in faces.iter() {
                            tr {
                                td { class: "mono", "{face.face_id}" }
                                td {
                                    span { class: "{face.kind_badge_class()}", "{face.kind_label()}" }
                                }
                                td { class: "mono",
                                    "{face.remote_uri.as_deref().unwrap_or(\"—\")}"
                                }
                                td { class: "mono",
                                    "{face.local_uri.as_deref().unwrap_or(\"—\")}"
                                }
                                td { "{face.persistency}" }
                            }
                        }
                    }
                }
            }
        }

        // ── Quick actions ───────────────────────────────────────────────────
        div { class: "section",
            div { class: "section-title", "Quick Actions" }
            div { style: "display:flex;gap:10px;",
                button {
                    class: "btn btn-secondary",
                    onclick: move |_| { ctx.cmd.send(DashCmd::Reconnect); },
                    "Refresh"
                }
                if *ctx.conn.read() == ConnState::Connected {
                    button {
                        class: "btn btn-danger",
                        onclick: move |_| {
                            ctx.cmd.send(DashCmd::Shutdown);
                        },
                        "Shutdown Router"
                    }
                }
            }
        }

        // ── Router process ──────────────────────────────────────────────────
        div { class: "section",
            div { class: "section-title", "Router Process" }
            div { style: "display:flex;align-items:center;gap:12px;margin-bottom:12px;",
                {
                    let running = *ROUTER_RUNNING.read();
                    rsx! {
                        span {
                            class: if running { "badge badge-green" } else { "badge badge-gray" },
                            if running { "Running" } else { "Stopped" }
                        }
                        if !running {
                            button {
                                class: "btn btn-primary btn-sm",
                                onclick: move |_| ctx.router_cmd.send(RouterCmd::Start),
                                "Start Router"
                            }
                        } else {
                            button {
                                class: "btn btn-danger btn-sm",
                                onclick: move |_| ctx.router_cmd.send(RouterCmd::Stop),
                                "Stop Router"
                            }
                        }
                    }
                }
            }
            {
                let log = ROUTER_LOG.read();
                if log.is_empty() {
                    rsx! {
                        div { class: "empty", style: "padding:8px 0;",
                            "No log output."
                        }
                    }
                } else {
                    let entries: Vec<_> = log.iter().rev().take(8).cloned().collect();
                    rsx! {
                        div { style: "background:#0d1117;border:1px solid #21262d;border-radius:4px;padding:8px 10px;",
                            for entry in entries.into_iter().rev() {
                                div { class: "log-entry",
                                    span {
                                        class: "log-lvl",
                                        style: "color:{entry.level.color()};background:{entry.level.bg()};",
                                        "{entry.level.as_str()}"
                                    }
                                    span { class: "log-target", "{entry.target}" }
                                    span { class: "log-msg", "{entry.message}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── Helper component ──────────────────────────────────────────────────────────

#[component]
fn StatCard(label: &'static str, value: u64) -> Element {
    rsx! {
        div { class: "card",
            div { class: "card-label", "{label}" }
            div { class: "card-value", "{value}" }
        }
    }
}
