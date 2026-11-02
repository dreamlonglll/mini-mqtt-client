//! "Add subscription" dialog.

use super::common::{dialog_footer, form_row, input_value, notify, segmented};
use crate::state::{AppState, ToastKind};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{ActiveTheme as _, WindowExt as _, v_flex};
use gpui_kit::{prelude::*, *};

struct SubscriptionForm {
    state: Entity<AppState>,
    server_id: i64,
    topic: Entity<InputState>,
    qos: u8,
}

impl SubscriptionForm {
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let topic = input_value(&self.topic, cx);
        let (server_id, qos) = (self.server_id, self.qos);
        match self
            .state
            .update(cx, |state, cx| state.add_subscription(server_id, topic, qos, cx))
        {
            Ok(()) => {
                notify(window, cx, ToastKind::Success, "订阅已添加");
                true
            }
            Err(err) => {
                notify(window, cx, ToastKind::Warning, err);
                false
            }
        }
    }
}

impl Render for SubscriptionForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        v_flex()
            .gap_4()
            .child(form_row("Topic", true, Input::new(&self.topic), cx))
            .child(form_row(
                "QoS",
                false,
                segmented(
                    "sub-qos",
                    &["QoS 0", "QoS 1", "QoS 2"],
                    self.qos as usize,
                    move |ix, _, cx| {
                        entity.update(cx, |this, cx| {
                            this.qos = ix as u8;
                            cx.notify();
                        })
                    },
                ),
                cx,
            ))
            .child(
                div()
                    .pl(px(108.))
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("支持通配符：+ 匹配单个层级（如 sensor/+/temp），# 匹配多个层级（如 device/#）"),
            )
    }
}

pub fn open(state: Entity<AppState>, server_id: i64, window: &mut Window, cx: &mut App) {
    let topic = cx.new(|cx| InputState::new(window, cx).placeholder("例如: device/+/status 或 test/#"));
    let form = cx.new(|_| SubscriptionForm {
        state,
        server_id,
        topic: topic.clone(),
        qos: 0,
    });
    let first = std::cell::Cell::new(true);
    window.open_dialog(cx, move |dialog, window, cx| {
        if first.replace(false) {
            topic.update(cx, |s, cx| s.focus(window, cx));
        }
        let save_form = form.clone();
        dialog
            .title("添加订阅")
            .w(px(520.))
            .child(form.clone())
            .footer(dialog_footer("订阅"))
            .on_ok(move |_, window, cx| save_form.update(cx, |form, cx| form.save(window, cx)))
    });
}
