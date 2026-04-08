use dioxus::prelude::*;
use ndn_config::{
    CsConfig, DiscoveryTomlConfig, EngineConfig, FaceConfig, ForwarderConfig,
    LoggingConfig, ManagementConfig, RouteConfig, SecurityConfig,
};

use crate::app::{AppCtx, DashCmd};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfigSection {
    Engine,
    ContentStore,
    Management,
    Security,
    Discovery,
    Logging,
}

#[component]
pub fn Config() -> Element {
    let ctx = use_context::<AppCtx>();
    let config_toml = ctx.config_toml.read().clone();

    let parsed: Option<ForwarderConfig> = if config_toml.is_empty() {
        None
    } else {
        config_toml.parse().ok()
    };

    // Export panel state
    let mut show_export = use_signal(|| false);
    let mut import_text = use_signal(String::new);
    let mut import_error = use_signal(|| Option::<String>::None);
    let mut export_text = use_signal(String::new);

    // Current editing section
    let mut editing: Signal<Option<ConfigSection>> = use_signal(|| None);
    let mut edu_dismissed = use_signal(|| false);

    rsx! {
        // ── Education snippet (B7): identity IS address ──────────────────────
        if !*edu_dismissed.read() {
            div { class: "edu-card",
                div { style: "display:flex;gap:12px;align-items:flex-start;",
                    div { style: "flex-shrink:0;padding-top:2px;",
                        div { class: "signed-packet", "did:ndn:/your/name" }
                    }
                    div { style: "flex:1;",
                        div { style: "display:flex;justify-content:space-between;align-items:flex-start;",
                            div { style: "font-size:13px;font-weight:600;color:#a371f7;margin-bottom:4px;",
                                "Your identity IS your address"
                            }
                            button {
                                style: "background:none;border:none;color:#8b949e;cursor:pointer;font-size:13px;padding:0;",
                                onclick: move |_| edu_dismissed.set(true),
                                "✕"
                            }
                        }
                        div { style: "font-size:12px;color:#8b949e;line-height:1.6;",
                            "In NDN, "
                            strong { style: "color:#c9d1d9;", "packets are addressed by name, not IP." }
                            " Your router's NDN name (configured below under "
                            span { style: "color:#a371f7;", "Security → router_name" }
                            ") becomes your DID — a globally unique, portable identity "
                            "that moves with you across networks."
                        }
                    }
                }
            }
        }

        if config_toml.is_empty() {
            div { class: "section",
                div { class: "empty",
                    "Config not loaded. "
                    button {
                        class: "btn btn-secondary btn-sm",
                        onclick: move |_| ctx.cmd.send(DashCmd::RefreshConfig),
                        "Load Config"
                    }
                }
            }
        } else if parsed.is_none() {
            div { class: "section",
                div { class: "empty", style: "color:#f85149;",
                    "Failed to parse config TOML. Raw:"
                }
                pre { style: "font-size:11px;color:#8b949e;overflow:auto;max-height:200px;", "{config_toml}" }
            }
        } else {
            {
                let cfg = parsed.unwrap();

                // ── Toolbar ───────────────────────────────────────────────
                rsx! {
                    div { style: "display:flex;gap:8px;margin-bottom:16px;align-items:center;",
                        button {
                            class: "btn btn-secondary",
                            onclick: move |_| ctx.cmd.send(DashCmd::RefreshConfig),
                            "↻ Reload"
                        }
                        button {
                            class: "btn btn-primary",
                            onclick: {
                                let toml = config_toml.clone();
                                move |_| {
                                    export_text.set(toml.clone());
                                    show_export.set(true);
                                }
                            },
                            "Export TOML"
                        }
                        button {
                            class: "btn btn-secondary",
                            onclick: move |_| {
                                import_text.set(String::new());
                                import_error.set(None);
                                show_export.set(false);
                            },
                            "Import TOML"
                        }
                        span { style: "font-size:11px;color:#8b949e;margin-left:auto;",
                            "Changes require router restart. Export the TOML and restart with --config."
                        }
                    }

                    // ── Restart-required banner ───────────────────────────
                    if editing.read().is_some() {
                        div { class: "restart-banner",
                            span { style: "font-size:14px;", "⚠" }
                            span {
                                "Settings are being edited. "
                                strong { "Export the TOML and restart the router" }
                                " to apply changes — live editing is not supported."
                            }
                            button {
                                class: "btn btn-secondary btn-sm",
                                style: "margin-left:auto;",
                                onclick: move |_| { editing.set(None); },
                                "Dismiss"
                            }
                        }
                    }

                    // ── Export panel ──────────────────────────────────────
                    if *show_export.read() {
                        div { class: "section",
                            div { style: "display:flex;justify-content:space-between;align-items:center;margin-bottom:8px;",
                                div { class: "section-title", "TOML Export" }
                                button {
                                    class: "btn btn-secondary btn-sm",
                                    onclick: move |_| show_export.set(false),
                                    "✕ Close"
                                }
                            }
                            textarea {
                                style: "width:100%;height:300px;background:#0d1117;border:1px solid #30363d;color:#c9d1d9;font-family:'SF Mono',monospace;font-size:12px;padding:10px;border-radius:4px;resize:vertical;",
                                readonly: true,
                                value: "{export_text}",
                            }
                            div { style: "margin-top:8px;font-size:12px;color:#8b949e;",
                                "Save this TOML to a file and launch ndn-router with: "
                                code { style: "color:#58a6ff;", "ndn-router --config /path/to/config.toml" }
                            }
                        }
                    }

                    // ── Import panel ──────────────────────────────────────
                    if !*show_export.read() {
                        div { class: "section",
                            div { class: "section-title", "Import Config" }
                            textarea {
                                style: "width:100%;height:200px;background:#0d1117;border:1px solid #30363d;color:#c9d1d9;font-family:'SF Mono',monospace;font-size:12px;padding:10px;border-radius:4px;resize:vertical;",
                                placeholder: "Paste TOML config here to preview and validate…",
                                value: "{import_text}",
                                oninput: move |e| {
                                    let text = e.value();
                                    import_text.set(text.clone());
                                    let result: Result<ForwarderConfig, _> = text.parse();
                                    import_error.set(result.err().map(|e| e.to_string()));
                                },
                            }
                            if let Some(ref err) = *import_error.read() {
                                div { style: "color:#f85149;font-size:12px;margin-top:6px;",
                                    "Parse error: {err}"
                                }
                            } else if !import_text.read().is_empty() {
                                div { style: "color:#3fb950;font-size:12px;margin-top:6px;",
                                    "✓ Valid TOML config"
                                }
                            }
                        }
                    }

                    // ── Engine section ────────────────────────────────────
                    {render_engine_section(cfg.engine.clone(), editing)}

                    // ── Content Store section ─────────────────────────────
                    {render_cs_section(cfg.cs.clone(), editing)}

                    // ── Management section ────────────────────────────────
                    {render_management_section(cfg.management.clone(), editing)}

                    // ── Security section ──────────────────────────────────
                    {render_security_section(cfg.security.clone(), editing)}

                    // ── Discovery section ─────────────────────────────────
                    {render_discovery_section(cfg.discovery.clone(), editing)}

                    // ── Logging section ───────────────────────────────────
                    {render_logging_section(cfg.logging.clone(), ctx, editing)}

                    // ── Faces section ─────────────────────────────────────
                    {render_faces_section(&cfg.faces)}

                    // ── Static Routes section ─────────────────────────────
                    {render_routes_section(&cfg.routes)}
                }
            }
        }
    }
}

// ── Section helpers ───────────────────────────────────────────────────────────

fn section_header(title: &str, section: ConfigSection, mut editing: Signal<Option<ConfigSection>>) -> Element {
    let is_open = *editing.read() == Some(section);
    rsx! {
        div { style: "display:flex;justify-content:space-between;align-items:center;margin-bottom:12px;",
            div { class: "section-title", "{title}" }
            button {
                class: "btn btn-secondary btn-sm",
                onclick: move |_| {
                    if *editing.read() == Some(section) {
                        editing.set(None);
                    } else {
                        editing.set(Some(section));
                    }
                },
                if is_open { "▲ Collapse" } else { "▼ Edit" }
            }
        }
    }
}

fn restart_badge() -> Element {
    rsx! { span { class: "badge badge-yellow", style: "font-size:10px;margin-left:6px;", "restart required" } }
}

fn live_badge() -> Element {
    rsx! { span { class: "badge badge-green", style: "font-size:10px;margin-left:6px;", "live" } }
}

fn kv_row(key: &str, val: impl std::fmt::Display, badge: Element) -> Element {
    let val = val.to_string();
    rsx! {
        tr {
            td { style: "color:#8b949e;font-size:12px;padding:5px 12px;width:220px;", "{key}" }
            td { class: "mono", style: "padding:5px 12px;", "{val}" }
            td { style: "padding:5px 12px;", {badge} }
        }
    }
}

// ── Engine ────────────────────────────────────────────────────────────────────

fn render_engine_section(engine: EngineConfig, editing: Signal<Option<ConfigSection>>) -> Element {
    let is_open = *editing.read() == Some(ConfigSection::Engine);

    let mut threads = use_signal(|| engine.pipeline_threads.to_string());
    let mut chan_cap = use_signal(|| engine.pipeline_channel_cap.to_string());

    rsx! {
        div { class: "section",
            {section_header("Engine", ConfigSection::Engine, editing)}

            table { style: "width:100%;",
                tbody {
                    {kv_row("Pipeline threads", if engine.pipeline_threads == 0 { "0 (auto)".to_string() } else { engine.pipeline_threads.to_string() }, restart_badge())}
                    {kv_row("Pipeline channel depth", engine.pipeline_channel_cap, restart_badge())}
                }
            }

            if is_open {
                div { class: "form-row", style: "margin-top:14px;",
                    div { class: "form-group",
                        label { "Pipeline threads (0 = auto)" }
                        input {
                            r#type: "number", min: "0", max: "64", style: "width:100px;",
                            value: "{threads}",
                            oninput: move |e| threads.set(e.value()),
                        }
                    }
                    div { class: "form-group",
                        label { "Channel depth" }
                        input {
                            r#type: "number", min: "64", max: "65536", style: "width:120px;",
                            value: "{chan_cap}",
                            oninput: move |e| chan_cap.set(e.value()),
                        }
                    }
                    div { style: "display:flex;align-items:flex-end;",
                        span { style: "font-size:12px;color:#8b949e;",
                            "Changes take effect after router restart."
                        }
                    }
                }
            }
        }
    }
}

// ── Content Store ─────────────────────────────────────────────────────────────

fn render_cs_section(cs: CsConfig, editing: Signal<Option<ConfigSection>>) -> Element {
    let is_open = *editing.read() == Some(ConfigSection::ContentStore);

    rsx! {
        div { class: "section",
            {section_header("Content Store", ConfigSection::ContentStore, editing)}

            table { style: "width:100%;",
                tbody {
                    {kv_row("Variant", &cs.variant, restart_badge())}
                    {kv_row("Capacity", format!("{} MB", cs.capacity_mb), restart_badge())}
                    if let Some(shards) = cs.shards {
                        {kv_row("Shards", shards, restart_badge())}
                    }
                    {kv_row("Admission policy", &cs.admission_policy, restart_badge())}
                }
            }

            if is_open {
                div { class: "form-row", style: "margin-top:14px;",
                    div { class: "form-group",
                        label { "Variant" }
                        select {
                            option { value: "lru",         selected: cs.variant == "lru",         "LRU" }
                            option { value: "sharded-lru", selected: cs.variant == "sharded-lru", "Sharded LRU" }
                            option { value: "null",        selected: cs.variant == "null",        "Null (disabled)" }
                        }
                    }
                    div { class: "form-group",
                        label { "Capacity (MB)" }
                        input {
                            r#type: "number", min: "0", max: "65536", style: "width:120px;",
                            value: "{cs.capacity_mb}",
                        }
                    }
                    div { class: "form-group",
                        label { "Admission policy" }
                        select {
                            option { value: "default",   selected: cs.admission_policy == "default",   "Satisfy PIT only" }
                            option { value: "admit-all", selected: cs.admission_policy == "admit-all", "Admit all Data" }
                        }
                    }
                    div { style: "display:flex;align-items:flex-end;",
                        span { style: "font-size:12px;color:#8b949e;", "Requires restart." }
                    }
                }
            }
        }
    }
}

// ── Management ────────────────────────────────────────────────────────────────

fn render_management_section(mgmt: ManagementConfig, editing: Signal<Option<ConfigSection>>) -> Element {
    let is_open = *editing.read() == Some(ConfigSection::Management);

    rsx! {
        div { class: "section",
            {section_header("Management", ConfigSection::Management, editing)}

            table { style: "width:100%;",
                tbody {
                    {kv_row("Transport", &mgmt.transport, restart_badge())}
                    {kv_row("Face socket", &mgmt.face_socket, restart_badge())}
                    {kv_row("Bypass socket", &mgmt.bypass_socket, restart_badge())}
                }
            }

            if is_open {
                div { class: "form-row", style: "margin-top:14px;",
                    div { class: "form-group",
                        label { "Transport" }
                        select {
                            option { value: "ndn",    selected: mgmt.transport == "ndn",    "NDN (recommended)" }
                            option { value: "bypass", selected: mgmt.transport == "bypass", "Bypass JSON" }
                        }
                    }
                    div { class: "form-group", style: "flex:1;",
                        label { "Face socket path" }
                        input { r#type: "text", value: "{mgmt.face_socket}", }
                    }
                    div { class: "form-group", style: "flex:1;",
                        label { "Bypass socket path" }
                        input {
                            r#type: "text", value: "{mgmt.bypass_socket}",
                            disabled: mgmt.transport != "bypass",
                        }
                    }
                }
            }
        }
    }
}

// ── Security ──────────────────────────────────────────────────────────────────

fn render_security_section(sec: SecurityConfig, editing: Signal<Option<ConfigSection>>) -> Element {
    let is_open = *editing.read() == Some(ConfigSection::Security);

    rsx! {
        div { class: "section",
            {section_header("Security", ConfigSection::Security, editing)}

            table { style: "width:100%;",
                tbody {
                    {kv_row("Identity", sec.identity.as_deref().unwrap_or("(none)"), restart_badge())}
                    {kv_row("PIB path", sec.pib_path.as_deref().unwrap_or("(default)"), restart_badge())}
                    {kv_row("Trust anchor", sec.trust_anchor.as_deref().unwrap_or("(none)"), restart_badge())}
                    {kv_row("Profile", &sec.profile, restart_badge())}
                    {kv_row("Require signed", sec.require_signed, restart_badge())}
                    {kv_row("Auto-init", sec.auto_init, restart_badge())}
                }
            }

            if is_open {
                div { class: "form-row", style: "margin-top:14px;",
                    div { class: "form-group", style: "flex:1;",
                        label { "Identity name" }
                        input { r#type: "text", placeholder: "/ndn/site/router1",
                            value: "{sec.identity.clone().unwrap_or_default()}",
                        }
                    }
                    div { class: "form-group", style: "flex:1;",
                        label { "PIB path" }
                        input { r#type: "text", placeholder: "~/.ndn/pib",
                            value: "{sec.pib_path.clone().unwrap_or_default()}",
                        }
                    }
                }
                div { class: "form-row",
                    div { class: "form-group", style: "flex:1;",
                        label { "Trust anchor file" }
                        input { r#type: "text", placeholder: "/etc/ndn/trust-anchor.cert",
                            value: "{sec.trust_anchor.clone().unwrap_or_default()}",
                        }
                    }
                    div { class: "form-group",
                        label { "Profile" }
                        select {
                            option { value: "default",       selected: sec.profile == "default",       "Default (chain validation)" }
                            option { value: "accept-signed", selected: sec.profile == "accept-signed", "Accept signed (no chain)" }
                            option { value: "disabled",      selected: sec.profile == "disabled",      "Disabled (lab only)" }
                        }
                    }
                    div { class: "form-group",
                        label { "Require signed" }
                        input { r#type: "checkbox", checked: sec.require_signed, }
                    }
                    div { class: "form-group",
                        label { "Auto-init" }
                        input { r#type: "checkbox", checked: sec.auto_init, }
                    }
                }
            }
        }
    }
}

// ── Discovery ─────────────────────────────────────────────────────────────────

fn render_discovery_section(disc: DiscoveryTomlConfig, editing: Signal<Option<ConfigSection>>) -> Element {
    let is_open = *editing.read() == Some(ConfigSection::Discovery);
    let enabled = disc.enabled();

    rsx! {
        div { class: "section",
            {section_header("Discovery", ConfigSection::Discovery, editing)}

            if !enabled {
                div { style: "color:#8b949e;font-size:13px;margin-bottom:8px;",
                    "Discovery is disabled. Set "
                    code { "node_name" }
                    " to enable."
                }
            }

            table { style: "width:100%;",
                tbody {
                    {kv_row("Node name", disc.node_name.as_deref().unwrap_or("(disabled)"), restart_badge())}
                    {kv_row("Profile", disc.profile.as_deref().unwrap_or("(default)"), restart_badge())}
                    {kv_row("Transport", disc.discovery_transport.as_deref().unwrap_or("udp"), restart_badge())}
                    {kv_row("Hello base interval", disc.hello_interval_base_ms.map(|v| format!("{v} ms")).unwrap_or("(default)".into()), restart_badge())}
                    {kv_row("Hello max interval", disc.hello_interval_max_ms.map(|v| format!("{v} ms")).unwrap_or("(default)".into()), restart_badge())}
                    {kv_row("Liveness miss count", disc.liveness_miss_count.map(|v| v.to_string()).unwrap_or("(default)".into()), restart_badge())}
                    {kv_row("Gossip fanout", disc.gossip_fanout.map(|v| v.to_string()).unwrap_or("(default)".into()), restart_badge())}
                    {kv_row("Relay records", disc.relay_records.map(|v| v.to_string()).unwrap_or("(default)".into()), restart_badge())}
                    {kv_row("Auto-FIB cost", disc.auto_fib_cost.map(|v| v.to_string()).unwrap_or("(default)".into()), restart_badge())}
                    {kv_row("Served prefixes", disc.served_prefixes.join(", "), restart_badge())}
                }
            }

            if is_open {
                div { class: "form-row", style: "margin-top:14px;",
                    div { class: "form-group", style: "flex:1;",
                        label { "Node name (empty = disable discovery)" }
                        input { r#type: "text", placeholder: "/ndn/site/router1",
                            value: "{disc.node_name.clone().unwrap_or_default()}",
                        }
                    }
                    div { class: "form-group",
                        label { "Profile" }
                        select {
                            option { value: "static",        selected: disc.profile.as_deref() == Some("static"),        "Static" }
                            option { value: "lan",           selected: disc.profile.as_deref() == Some("lan") || disc.profile.is_none(), "LAN" }
                            option { value: "campus",        selected: disc.profile.as_deref() == Some("campus"),        "Campus" }
                            option { value: "mobile",        selected: disc.profile.as_deref() == Some("mobile"),        "Mobile" }
                            option { value: "high-mobility", selected: disc.profile.as_deref() == Some("high-mobility"), "High-mobility" }
                            option { value: "asymmetric",    selected: disc.profile.as_deref() == Some("asymmetric"),    "Asymmetric" }
                        }
                    }
                    div { class: "form-group",
                        label { "Transport" }
                        select {
                            option { value: "udp",   selected: disc.discovery_transport.as_deref() != Some("ether") && disc.discovery_transport.as_deref() != Some("both"), "UDP multicast" }
                            option { value: "ether", selected: disc.discovery_transport.as_deref() == Some("ether"), "Ethernet multicast" }
                            option { value: "both",  selected: disc.discovery_transport.as_deref() == Some("both"),  "Both" }
                        }
                    }
                }
                div { class: "form-row",
                    div { class: "form-group",
                        label { "Hello base interval (ms)" }
                        input { r#type: "number", min: "100", style: "width:120px;",
                            value: "{disc.hello_interval_base_ms.map(|v| v.to_string()).unwrap_or_default()}",
                            placeholder: "(profile default)",
                        }
                    }
                    div { class: "form-group",
                        label { "Hello max interval (ms)" }
                        input { r#type: "number", min: "1000", style: "width:120px;",
                            value: "{disc.hello_interval_max_ms.map(|v| v.to_string()).unwrap_or_default()}",
                            placeholder: "(profile default)",
                        }
                    }
                    div { class: "form-group",
                        label { "Liveness miss count" }
                        input { r#type: "number", min: "1", max: "20", style: "width:80px;",
                            value: "{disc.liveness_miss_count.map(|v| v.to_string()).unwrap_or_default()}",
                            placeholder: "(profile default)",
                        }
                    }
                    div { class: "form-group",
                        label { "Gossip fanout (0=off)" }
                        input { r#type: "number", min: "0", max: "10", style: "width:80px;",
                            value: "{disc.gossip_fanout.map(|v| v.to_string()).unwrap_or_default()}",
                            placeholder: "(profile default)",
                        }
                    }
                    div { class: "form-group",
                        label { "Auto-FIB cost" }
                        input { r#type: "number", min: "0", style: "width:80px;",
                            value: "{disc.auto_fib_cost.map(|v| v.to_string()).unwrap_or_default()}",
                            placeholder: "(profile default)",
                        }
                    }
                }
            }
        }
    }
}

// ── Logging ───────────────────────────────────────────────────────────────────

fn render_logging_section(log: LoggingConfig, _ctx: AppCtx, editing: Signal<Option<ConfigSection>>) -> Element {
    let is_open = *editing.read() == Some(ConfigSection::Logging);

    let mut threads = use_signal(|| log.level.clone());
    let mut log_file = use_signal(|| log.file.clone().unwrap_or_default());

    rsx! {
        div { class: "section",
            {section_header("Logging", ConfigSection::Logging, editing)}

            table { style: "width:100%;",
                tbody {
                    {kv_row("Level", &log.level, live_badge())}
                    {kv_row("Log file", log.file.as_deref().unwrap_or("(stderr only)"), restart_badge())}
                }
            }

            if is_open {
                div { class: "form-row", style: "margin-top:14px;",
                    div { class: "form-group",
                        label { "Log level " {live_badge()} }
                        select {
                            onchange: move |e| threads.set(e.value()),
                            option { value: "error", selected: log.level == "error", "error" }
                            option { value: "warn",  selected: log.level == "warn",  "warn" }
                            option { value: "info",  selected: log.level == "info",  "info" }
                            option { value: "debug", selected: log.level == "debug", "debug" }
                            option { value: "trace", selected: log.level == "trace", "trace" }
                        }
                    }
                    div { class: "form-group", style: "flex:1;",
                        label { "Custom filter (overrides level)" }
                        input { r#type: "text", placeholder: "info,ndn_engine=debug",
                            value: if log.level.contains(',') || log.level.contains('=') { log.level.clone() } else { String::new() },
                        }
                    }
                    div { class: "form-group", style: "flex:1;",
                        label { "Log file " {restart_badge()} }
                        input { r#type: "text", placeholder: "/var/log/ndn/router.log",
                            value: "{log_file}",
                            oninput: move |e| log_file.set(e.value()),
                        }
                    }
                    div { style: "display:flex;align-items:flex-end;gap:8px;",
                        div { style: "font-size:12px;color:#8b949e;",
                            "Log level applies live. File path requires restart."
                        }
                    }
                }
            }
        }
    }
}

// ── Faces (read-only display) ─────────────────────────────────────────────────

fn render_faces_section(faces: &[FaceConfig]) -> Element {
    rsx! {
        div { class: "section",
            div { class: "section-title",
                "Startup Faces "
                {restart_badge()}
            }
            if faces.is_empty() {
                div { class: "empty", "No startup faces configured. Faces can be created at runtime via the Faces tab." }
            } else {
                div { style: "display:flex;flex-direction:column;gap:8px;",
                    for (i, face) in faces.iter().enumerate() {
                        {render_face_card(i, face)}
                    }
                }
            }
            div { style: "margin-top:8px;font-size:12px;color:#8b949e;",
                "Startup faces are configured in the TOML config file and created automatically when the router starts."
            }
        }
    }
}

fn render_face_card(i: usize, face: &FaceConfig) -> Element {
    let (kind_label, details) = match face {
        FaceConfig::Udp { bind, remote } => (
            "UDP",
            format!("bind={} remote={}", bind.as_deref().unwrap_or("any"), remote.as_deref().unwrap_or("(unicast)"))
        ),
        FaceConfig::Tcp { bind, remote } => (
            "TCP",
            format!("bind={} remote={}", bind.as_deref().unwrap_or("any"), remote.as_deref().unwrap_or("(listen)"))
        ),
        FaceConfig::Multicast { group, port, interface } => (
            "Multicast",
            format!("group={group}:{port} iface={}", interface.as_deref().unwrap_or("any"))
        ),
        FaceConfig::Unix { path } => (
            "Unix",
            format!("path={}", path.as_deref().unwrap_or("(default)"))
        ),
        FaceConfig::WebSocket { bind, url } => (
            "WebSocket",
            format!("bind={} url={}", bind.as_deref().unwrap_or("(none)"), url.as_deref().unwrap_or("(none)"))
        ),
        FaceConfig::Serial { path, baud } => (
            "Serial",
            format!("path={path} baud={baud}")
        ),
        FaceConfig::EtherMulticast { interface } => (
            "Ether-Multicast",
            format!("iface={interface}")
        ),
    };

    rsx! {
        div { style: "background:#0d1117;border:1px solid #30363d;border-radius:6px;padding:10px 14px;display:flex;align-items:center;gap:12px;",
            span { class: "badge badge-blue", "{kind_label}" }
            span { style: "font-size:12px;color:#8b949e;", "face[{i}]" }
            span { class: "mono", style: "font-size:12px;", "{details}" }
        }
    }
}

// ── Static Routes (read-only display) ────────────────────────────────────────

fn render_routes_section(routes: &[RouteConfig]) -> Element {
    rsx! {
        div { class: "section",
            div { class: "section-title",
                "Startup Routes "
                {restart_badge()}
            }
            if routes.is_empty() {
                div { class: "empty", "No startup routes configured. Routes can be added at runtime via the Routes tab." }
            } else {
                table {
                    thead {
                        tr {
                            th { "Prefix" }
                            th { "Face Index" }
                            th { "Cost" }
                        }
                    }
                    tbody {
                        for route in routes.iter() {
                            tr {
                                td { class: "mono", "{route.prefix}" }
                                td { class: "mono", "{route.face}" }
                                td { class: "mono", "{route.cost}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
