use dioxus::prelude::*;

use crate::app::{AppCtx, DashCmd};

#[component]
pub fn Routes() -> Element {
    let ctx = use_context::<AppCtx>();
    let routes = ctx.routes.read();
    let faces = ctx.faces.read();

    let mut new_prefix: Signal<String> = use_signal(String::new);
    let mut new_face_id: Signal<String> = use_signal(String::new);
    let mut new_cost: Signal<String> = use_signal(|| "10".to_string());

    rsx! {
        div { class: "section",
            div { class: "section-title", "FIB Routes" }
            if routes.is_empty() {
                div { class: "empty", "No routes in FIB." }
            } else {
                table {
                    thead {
                        tr {
                            th { "Prefix" }
                            th { "Nexthops" }
                            th { "" }
                        }
                    }
                    tbody {
                        for entry in routes.iter() {
                            {
                                let prefix = entry.prefix.clone();
                                let nexthops_display = entry.nexthops.iter()
                                    .map(|nh| {
                                        let uri = faces.iter()
                                            .find(|f| f.face_id == nh.face_id)
                                            .and_then(|f| f.remote_uri.as_deref())
                                            .map(|u| format!(" ({})", u))
                                            .unwrap_or_default();
                                        format!("face {}{} cost {}", nh.face_id, uri, nh.cost)
                                    })
                                    .collect::<Vec<_>>()
                                    .join("  |  ");
                                // Use first nexthop face_id for remove button
                                let first_face = entry.nexthops.first().map(|nh| nh.face_id);
                                rsx! {
                                    tr {
                                        td { class: "mono", "{prefix}" }
                                        td { class: "mono", "{nexthops_display}" }
                                        td {
                                            if let Some(face_id) = first_face {
                                                button {
                                                    class: "btn btn-danger btn-sm",
                                                    onclick: move |_| {
                                                        ctx.cmd.send(DashCmd::RouteRemove {
                                                            prefix: prefix.clone(),
                                                            face_id,
                                                        });
                                                    },
                                                    "Remove"
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

            // ── Add route form ───────────────────────────────────────────────
            div { class: "form-row",
                div { class: "form-group",
                    label { r#for: "rt-prefix", "Name Prefix" }
                    input {
                        id: "rt-prefix",
                        r#type: "text",
                        placeholder: "/ndn",
                        value: "{new_prefix}",
                        oninput: move |e| new_prefix.set(e.value()),
                    }
                }
                div { class: "form-group",
                    label { r#for: "rt-face", "Face ID" }
                    input {
                        id: "rt-face",
                        r#type: "number",
                        placeholder: "1",
                        value: "{new_face_id}",
                        style: "width:80px",
                        oninput: move |e| new_face_id.set(e.value()),
                    }
                }
                div { class: "form-group",
                    label { r#for: "rt-cost", "Cost" }
                    input {
                        id: "rt-cost",
                        r#type: "number",
                        value: "{new_cost}",
                        style: "width:80px",
                        oninput: move |e| new_cost.set(e.value()),
                    }
                }
                button {
                    class: "btn btn-primary",
                    onclick: move |_| {
                        let prefix  = new_prefix.read().trim().to_string();
                        let face_id = new_face_id.read().trim().parse::<u64>().unwrap_or(0);
                        let cost    = new_cost.read().trim().parse::<u64>().unwrap_or(10);
                        if !prefix.is_empty() {
                            ctx.cmd.send(DashCmd::RouteAdd { prefix, face_id, cost });
                            new_prefix.set(String::new());
                            new_face_id.set(String::new());
                        }
                    },
                    "Add Route"
                }
            }
        }
    }
}
