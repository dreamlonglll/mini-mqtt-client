//! Create / edit server dialog (tabs: 基本配置 / 认证 / 高级).

use super::common::{IconName, dialog_footer, form_row, input_value, notify, opt_value, segmented};
use crate::backend::models::{MqttServer, Transport};
use crate::state::{AppState, ToastKind};
use gpui_kit::component::button::Button;
use gpui_kit::component::input::{Input, InputState, NumberInput, Textarea, TextareaState};
use gpui_kit::component::radio::{Radio, RadioGroup};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{ActiveTheme as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::{prelude::*, *};

pub fn generate_client_id() -> String {
    let random = uuid::Uuid::new_v4().simple().to_string();
    format!("mqtt_{}_{}", chrono::Local::now().timestamp_millis(), &random[..8])
}

fn number_state(window: &mut Window, cx: &mut App, value: i64, min: f64, max: f64) -> Entity<InputState> {
    cx.new(|cx| {
        InputState::new(window, cx)
            .step(1.)
            .min(min)
            .max(max)
            .validate(|s, _| s.chars().all(|c| c.is_ascii_digit()))
            .default_value(value.to_string())
    })
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
        let name = text(window, cx, &server.name, "给这个连接起个名字");
        let host = text(window, cx, &server.host, "主机地址，例如 broker.emqx.io");
        let client_id = text(window, cx, server.client_id.as_deref().unwrap_or(""), "留空则自动生成");
        let username = text(window, cx, server.username.as_deref().unwrap_or(""), "可选");
        let password = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("可选")
                .masked(true)
                .default_value(server.password.clone().unwrap_or_default())
        });
        let port = number_state(window, cx, server.port as i64, 1., 65535.);
        let keep_alive = number_state(window, cx, server.keep_alive as i64, 0., 65535.);
        let ca_cert = pem_state(
            window,
            cx,
            &server.ca_cert,
            "-----BEGIN CERTIFICATE-----（留空则使用系统根证书）",
        );
        let client_cert = pem_state(
            window,
            cx,
            &server.client_cert,
            "-----BEGIN CERTIFICATE-----（可选，双向认证）",
        );
        let client_key = pem_state(
            window,
            cx,
            &server.client_key,
            "-----BEGIN PRIVATE KEY-----（可选，双向认证）",
        );
        Self {
            state,
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
            Some("请输入名称")
        } else if host.is_empty() {
            Some("请输入主机地址")
        } else if !(1..=65535).contains(&port) {
            Some("端口范围为 1-65535")
        } else if !(0..=65535).contains(&keep_alive) {
            Some("Keep Alive 范围为 0-65535")
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
        let base = self.existing.clone().unwrap_or_default();
        let server = MqttServer {
            id: base.id,
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
            created_at: base.created_at,
            updated_at: base.updated_at,
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
                        "服务器已添加"
                    } else {
                        "服务器已更新"
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
            .child(form_row("名称", true, Input::new(&self.name), cx))
            .child(form_row(
                "服务器地址",
                true,
                v_flex()
                    .gap_2()
                    .child(segmented(
                        "transport",
                        &["mqtt://", "mqtts://", "ws://", "wss://"],
                        transport_ix,
                        move |ix, window, cx| {
                            e_transport.update(cx, |this, cx| this.set_transport(Transport::ALL[ix], window, cx))
                        },
                    ))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(div().flex_1().child(Input::new(&self.host)))
                            .child(":")
                            .child(div().w(px(130.)).child(NumberInput::new(&self.port))),
                    )
                    .when(matches!(self.transport, Transport::Ws | Transport::Wss), |this| {
                        this.child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child("WebSocket 默认路径为 /mqtt，可在主机后追加路径，例如 broker.emqx.io/mqtt"),
                        )
                    }),
                cx,
            ))
            .child(form_row(
                "协议版本",
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
                    .child(div().flex_1().child(Input::new(&self.client_id)))
                    .child(
                        Button::new("regen-client-id")
                            .outline()
                            .icon(IconName::RefreshCw)
                            .tooltip("随机生成")
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
            .child(form_row("用户名", false, Input::new(&self.username), cx))
            .child(form_row("密码", false, Input::new(&self.password).mask_toggle(), cx))
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
                    .child(div().w(px(160.)).child(NumberInput::new(&self.keep_alive)))
                    .child(div().text_sm().text_color(cx.theme().muted_foreground).child("秒")),
                cx,
            ))
            .child(form_row(
                if self.v5 { "Clean Start" } else { "Clean Session" },
                false,
                div()
                    .pt_1p5()
                    .child(
                        Switch::new("clean-session")
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
                        .child("选择 mqtts:// 或 wss:// 后可配置 TLS 证书"),
                )
            })
            .when(tls, |this| {
                this.child(form_row("CA 证书", false, Textarea::new(&self.ca_cert), cx))
                    .child(form_row("客户端证书", false, Textarea::new(&self.client_cert), cx))
                    .child(form_row("客户端私钥", false, Textarea::new(&self.client_key), cx))
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
                    .child(Tab::new().label("基本配置"))
                    .child(Tab::new().label("认证"))
                    .child(Tab::new().label("高级")),
            )
            .child(div().min_h(px(250.)).child(content))
    }
}

pub fn open(state: Entity<AppState>, existing: Option<MqttServer>, window: &mut Window, cx: &mut App) {
    let title = if existing.is_some() {
        "编辑 Server"
    } else {
        "新增 Server"
    };
    let form = cx.new(|cx| ServerForm::new(state, existing, window, cx));
    window.open_dialog(cx, move |dialog, _, _| {
        let save_form = form.clone();
        dialog
            .title(title)
            .w(px(640.))
            .child(form.clone())
            .footer(dialog_footer("保存"))
            .on_ok(move |_, window, cx| save_form.update(cx, |form, cx| form.save(window, cx)))
    });
}
