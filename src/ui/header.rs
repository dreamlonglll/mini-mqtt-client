//! Server info bar above the message list (status, address, connect button).

use super::common::{Lucide, chip, status_color, status_dot};
use super::server_dialog;
use crate::backend::errors::friendly_mqtt_error;
use crate::backend::mqtt::ConnectionStatus;
use crate::state::AppState;
use gpui_kit::component::StyledExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::{prelude::*, *};

pub struct HeaderView {
    state: Entity<AppState>,
    _subscription: Subscription,
}

impl HeaderView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let subscription = cx.observe(&state, |_, _, cx| cx.notify());
        Self {
            state,
            _subscription: subscription,
        }
    }
}

impl Render for HeaderView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let Some(server) = state.active_server().cloned() else {
            return div().into_any_element();
        };
        let id = server.id.unwrap_or_default();
        let status = state.status(id);
        let theme = cx.theme().clone();
        let color = status_color(&status, cx);

        let connect_state = self.state.clone();
        let edit_state = self.state.clone();
        let edit_server = server.clone();
        let busy = status.is_busy();

        let connect_button = if busy {
            Button::new("disconnect")
                .danger()
                .small()
                .icon(Lucide::Unplug)
                .label(if status == ConnectionStatus::Connecting {
                    tr!("common.cancel")
                } else {
                    tr!("header.disconnect")
                })
                .on_click(move |_, _, cx| connect_state.update(cx, |state, cx| state.disconnect(id, cx)))
        } else {
            Button::new("connect")
                .primary()
                .small()
                .icon(Lucide::Plug)
                .label(tr!("header.connect"))
                .on_click(move |_, _, cx| connect_state.update(cx, |state, cx| state.connect(id, cx)))
        };

        let error = match &status {
            ConnectionStatus::Error(err) | ConnectionStatus::Reconnecting(err) => Some(friendly_mqtt_error(err)),
            _ => None,
        };

        h_flex()
            .flex_none()
            .h(px(60.))
            .px_4()
            .gap_3()
            .bg(theme.background)
            .border_b_1()
            .border_color(theme.border)
            .child(status_dot(&status, cx).size(px(10.)))
            .child(
                v_flex()
                    .min_w_0()
                    .child(div().font_semibold().truncate().child(server.name.clone()))
                    .child(
                        div()
                            .text_xs()
                            .truncate()
                            .font_family(theme.mono_font_family.clone())
                            .text_color(theme.muted_foreground)
                            .child(server.address()),
                    ),
            )
            .child(
                chip(status.label(), color, cx)
                    .text_sm()
                    .px_2()
                    .id("connection-status")
                    .role(Role::Status)
                    .aria_label(match &status {
                        ConnectionStatus::Error(err) => format!("{} {} {}", tr!("header.status"), status.label(), err),
                        _ => format!("{} {}", tr!("header.status"), status.label()),
                    }),
            )
            .when_some(error, |this, error| {
                this.child(
                    div()
                        .min_w_0()
                        .flex_shrink(1.)
                        .text_xs()
                        .truncate()
                        .text_color(theme.danger)
                        .child(error),
                )
            })
            .child(div().flex_1())
            .child(
                chip(
                    if server.is_v5() { "MQTT 5.0" } else { "MQTT 3.1.1" },
                    theme.muted_foreground,
                    cx,
                )
                .text_sm()
                .px_2(),
            )
            .child(
                chip(format!("Keep Alive: {}s", server.keep_alive), theme.primary, cx)
                    .text_sm()
                    .px_2(),
            )
            .child(connect_button)
            .child(
                Button::new("edit-server")
                    .accessibility_label(tr!("header.edit_server"))
                    .ghost()
                    .small()
                    .icon(Lucide::SquarePen)
                    .tooltip(tr!("header.edit_server"))
                    .on_click(move |_, window, cx| {
                        server_dialog::open(edit_state.clone(), Some(edit_server.clone()), window, cx)
                    }),
            )
            .into_any_element()
    }
}
