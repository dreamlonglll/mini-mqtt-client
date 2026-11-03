//! Create / edit server dialog (tabs: basic / authentication / advanced).

use super::common::{
    IconName, dialog_footer, form_row, input_value, notify, number_field, number_state, opt_value, segmented,
};
use crate::backend::models::{MqttServer, Transport};
use crate::state::{AppState, ToastKind};
use gpui_kit::component::button::Button;
use gpui_kit::component::input::{Input, InputState, Textarea, TextareaState};
use gpui_kit::component::radio::{Radio, RadioGroup};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{ActiveTheme as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::{prelude::*, *};

pub fn generate_client_id() -> String {
    let random = uuid::Uuid::new_v4().simple().to_string();
    format!("mqtt_{}_{}", chrono::Local::now().timestamp_millis(), &random[..8])
}

fn pem_state(window: &mut Window, cx: &mut App, value: &Option<String>, placeholder: &str) -> Entity<TextareaState> {
    let placeholder = placeholder.to_string();
    let value = value.clone().unwrap_or_default();
    cx.new(|cx| {
        TextareaState::new(window, cx)
            .rows(4)
            .placeholder(placeholder)
            .default_value(value)
    })
}

struct ServerForm {
    state: Entity<AppState>,
    existing: Option<MqttServer>,
    tab: usize,
    name: Entity<InputState>,
    host: Entity<InputState>,
    port: Entity<InputState>,
    client_id: Entity<InputState>,
    username: Entity<InputState>,
    password: Entity<InputState>,
    keep_alive: Entity<InputState>,
    ca_cert: Entity<TextareaState>,
    client_cert: Entity<TextareaState>,
    client_key: Entity<TextareaState>,
    client_key_password: Entity<InputState>,
    transport: Transport,
    v5: bool,
    clean_session: bool,
}

impl ServerForm {
    fn new(state: Entity<AppState>, existing: Option<MqttServer>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let server = existing.clone().unwrap_or_default();
        let text = |window: &mut Window, cx: &mut Context<Self>, value: &str, placeholder: &str| {
            let (value, placeholder) = (value.to_string(), placeholder.to_string());
            cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder(placeholder)
                    .default_value(value)
            })
        };
        let name = text(window, cx, &server.name, &tr!("server.name_placeholder"));
        let host = text(window, cx, &server.host, &tr!("server.host_placeholder"));
        let client_id = text(
            window,
            cx,
            server.client_id.as_deref().unwrap_or(""),
            &tr!("server.client_id_placeholder"),
        );
        let username = text(
            window,
            cx,
            server.username.as_deref().unwrap_or(""),
            &tr!("common.optional"),
        );
        let password = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(tr!("common.optional"))
                .masked(true)
                .default_value(server.password.clone().unwrap_or_default())
        });
        let port = number_state(window, cx, server.port as i64);
        let keep_alive = number_state(window, cx, server.keep_alive as i64);
        let ca_cert = pem_state(window, cx, &server.ca_cert, &tr!("server.ca_placeholder"));
        let client_cert = pem_state(window, cx, &server.client_cert, &tr!("server.cert_placeholder"));
        let client_key = pem_state(window, cx, &server.client_key, &tr!("server.key_placeholder"));
        let client_key_password = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(tr!("server.key_password_placeholder"))
                .masked(true)
                .default_value(server.client_key_password.clone().unwrap_or_default())
        });
        Self {
            state,
            client_key_password,
            transport: server.transport(),
            v5: server.is_v5(),
            clean_session: server.clean_session,
            existing,
            tab: 0,
            name,
            host,
            port,
            client_id,
            username,
            password,
            keep_alive,
            ca_cert,
            client_cert,
            client_key,
        }
    }

    fn set_transport(&mut self, transport: Transport, window: &mut Window, cx: &mut Context<Self>) {
        // Follow the default port of the scheme unless the user picked a custom one.
        let current: i32 = input_value(&self.port, cx).parse().unwrap_or(0);
        if current == 0 || Transport::ALL.iter().any(|t| t.default_port() == current) {
            let port = transport.default_port().to_string();
            self.port.update(cx, |s, cx| s.set_value(port, window, cx));
        }
        self.transport = transport;
        cx.notify();
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let name = input_value(&self.name, cx).trim().to_string();
        let host = input_value(&self.host, cx).trim().to_string();
        let port: i32 = input_value(&self.port, cx).trim().parse().unwrap_or(0);
        let keep_alive: i32 = input_value(&self.keep_alive, cx).trim().parse().unwrap_or(-1);
        let error = if name.is_empty() {
            Some(tr!("server.name_required"))
        } else if host.is_empty() {
            Some(tr!("server.host_required"))
        } else if !(1..=65535).contains(&port) {
            Some(tr!("server.port_range"))
        } else if !(0..=65535).contains(&keep_alive) {
            Some(tr!("server.keep_alive_range"))
        } else {
            None
        };
        if let Some(error) = error {
            notify(window, cx, ToastKind::Warning, error);
            return false;
        }

        let tls = self.transport.uses_tls();
        let pem = |state: &Entity<TextareaState>, cx: &App| {
            let v = state.read(cx).value().trim().to_string();
            (tls && !v.is_empty()).then_some(v)
        };
        // Start from the stored server so fields this form does not edit
        // (and keys written by other versions) are kept.
        let base = self.existing.clone().unwrap_or_default();
        let server = MqttServer {
            name,
            host,
            port,
            protocol_version: if self.v5 { "5.0" } else { "3.1.1" }.to_string(),
            protocol: Some(self.transport.scheme().to_string()),
            username: opt_value(&self.username, cx),
            password: Some(input_value(&self.password, cx)).filter(|p| !p.is_empty()),
            client_id: opt_value(&self.client_id, cx),
            keep_alive,
            clean_session: self.clean_session,
            use_tls: tls,
            ca_cert: pem(&self.ca_cert, cx),
            client_cert: pem(&self.client_cert, cx),
            client_key: pem(&self.client_key, cx),
            client_key_password: Some(input_value(&self.client_key_password, cx)).filter(|p| tls && !p.is_empty()),
            ..base
        };

        let is_new = server.id.is_none();
        let result = self.state.update(cx, |state, cx| {
            if is_new {
                state.create_server(server, cx).map(|_| ())
            } else {
                state.update_server(server, cx)
            }
        });
        match result {
            Ok(()) => {
                notify(
                    window,
                    cx,
                    ToastKind::Success,
                    if is_new {
                        tr!("server.added")
                    } else {
                        tr!("server.updated")
                    },
                );
                true
            }
            Err(err) => {
                notify(window, cx, ToastKind::Error, err);
                false
            }
        }
    }

    fn render_basic(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let transport_ix = Transport::ALL.iter().position(|t| *t == self.transport).unwrap_or(0);
        let e_transport = entity.clone();
        let e_version = entity.clone();
        let regen_id = self.client_id.clone();
        v_flex()
            .gap_4()
            .child(form_row(
                tr!("server.name"),
                true,
                Input::new(&self.name).aria_label(tr!("server.name_aria")),
                cx,
            ))
            .child(form_row(
                tr!("server.address"),
                true,
                v_flex()
                    .gap_2()
                    .child(segmented(
                        "transport",
                        &tr!("server.transport"),
                        &["mqtt://", "mqtts://", "ws://", "wss://"],
                        transport_ix,
                        move |ix, window, cx| {
                            e_transport.update(cx, |this, cx| this.set_transport(Transport::ALL[ix], window, cx))
                        },
                    ))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .child(Input::new(&self.host).aria_label(tr!("server.host"))),
                            )
                            .child(":")
                            .child(div().w(px(130.)).child(number_field(
                                "port",
                                &tr!("server.port"),
                                &self.port,
                                1,
                                1,
                                65535,
                            ))),
                    )
                    .when(matches!(self.transport, Transport::Ws | Transport::Wss), |this| {
                        this.child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(tr!("server.ws_hint")),
                        )
                    }),
                cx,
            ))
            .child(form_row(
                tr!("server.protocol_version"),
                false,
                div().pt_1p5().child(
                    RadioGroup::horizontal("protocol-version")
                        .selected_index(Some(if self.v5 { 1 } else { 0 }))
                        .on_click(move |ix, _, cx| {
                            let ix = *ix;
                            e_version.update(cx, |this, cx| {
                                this.v5 = ix == 1;
                                cx.notify();
                            })
                        })
                        .child(Radio::new("v311").label("MQTT 3.1.1"))
                        .child(Radio::new("v5").label("MQTT 5.0")),
                ),
                cx,
            ))
            .child(form_row(
                "Client ID",
                false,
                h_flex()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .child(Input::new(&self.client_id).aria_label("Client ID")),
                    )
                    .child(
                        Button::new("regen-client-id")
                            .accessibility_label(tr!("server.regen_client_id"))
                            .outline()
                            .icon(IconName::RefreshCw)
                            .tooltip(tr!("server.regen"))
                            .on_click(move |_, window, cx| {
                                regen_id.update(cx, |s, cx| s.set_value(generate_client_id(), window, cx))
                            }),
                    ),
                cx,
            ))
    }

    fn render_auth(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_4()
            .child(form_row(
                tr!("server.username"),
                false,
                Input::new(&self.username).aria_label(tr!("server.username")),
                cx,
            ))
            .child(form_row(
                tr!("server.password"),
                false,
                Input::new(&self.password)
                    .aria_label(tr!("server.password"))
                    .mask_toggle(),
                cx,
            ))
    }

    fn render_advanced(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let tls = self.transport.uses_tls();
        v_flex()
            .gap_4()
            .child(form_row(
                "Keep Alive",
                false,
                h_flex()
                    .gap_2()
                    .child(div().w(px(160.)).child(number_field(
                        "keep-alive",
                        "Keep Alive",
                        &self.keep_alive,
                        10,
                        0,
                        65535,
                    )))
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(tr!("server.seconds")),
                    ),
                cx,
            ))
            .child(form_row(
                if self.v5 { "Clean Start" } else { "Clean Session" },
                false,
                div().pt_1p5().child(
                    Switch::new("clean-session")
                        .accessibility_label("Clean Session")
                        .checked(self.clean_session)
                        .on_click(move |checked, _, cx| {
                            let checked = *checked;
                            entity.update(cx, |this, cx| {
                                this.clean_session = checked;
                                cx.notify();
                            })
                        }),
                ),
                cx,
            ))
            .when(!tls, |this| {
                this.child(
                    div()
                        .pl(px(108.))
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(tr!("server.tls_hint")),
                )
            })
            .when(tls, |this| {
                this.child(form_row(
                    tr!("server.ca_cert"),
                    false,
                    Textarea::new(&self.ca_cert).aria_label(tr!("server.ca_cert")),
                    cx,
                ))
                .child(form_row(
                    tr!("server.client_cert"),
                    false,
                    Textarea::new(&self.client_cert).aria_label(tr!("server.client_cert")),
                    cx,
                ))
                .child(form_row(
                    tr!("server.client_key"),
                    false,
                    Textarea::new(&self.client_key).aria_label(tr!("server.client_key")),
                    cx,
                ))
                .child(form_row(
                    tr!("server.key_password"),
                    false,
                    Input::new(&self.client_key_password)
                        .aria_label(tr!("server.key_password"))
                        .mask_toggle(),
                    cx,
                ))
            })
    }
}

impl Render for ServerForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let content = match self.tab {
            1 => self.render_auth(cx).into_any_element(),
            2 => self.render_advanced(cx).into_any_element(),
            _ => self.render_basic(cx).into_any_element(),
        };
        v_flex()
            .gap_4()
            .child(
                TabBar::new("server-tabs")
                    .underline()
                    .selected_index(self.tab)
                    .on_click(move |ix, _, cx| {
                        let ix = *ix;
                        entity.update(cx, |this, cx| {
                            this.tab = ix;
                            cx.notify();
                        })
                    })
                    .child(Tab::new().label(tr!("server.tab_basic")))
                    .child(Tab::new().label(tr!("server.tab_auth")))
                    .child(Tab::new().label(tr!("server.tab_advanced"))),
            )
            .child(div().min_h(px(250.)).child(content))
    }
}

pub fn open(state: Entity<AppState>, existing: Option<MqttServer>, window: &mut Window, cx: &mut App) {
    let title = if existing.is_some() {
        tr!("server.edit_title")
    } else {
        tr!("server.add_title")
    };
    let form = cx.new(|cx| ServerForm::new(state, existing, window, cx));
    window.open_dialog(cx, move |dialog, _, _| {
        let save_form = form.clone();
        dialog
            .title(title.clone())
            .w(px(640.))
            .child(form.clone())
            .footer(dialog_footer(tr!("common.save")))
            .on_ok(move |_, window, cx| save_form.update(cx, |form, cx| form.save(window, cx)))
    });
}
