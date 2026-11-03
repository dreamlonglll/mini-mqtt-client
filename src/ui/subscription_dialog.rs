//! Add / edit subscription dialog (topic, QoS, colour mark).

use super::common::{color_from, dialog_footer, form_row, input_value, notify, segmented};
use crate::backend::color::PRESET_COLORS;
use crate::backend::models::Subscription as TopicSubscription;
use crate::state::{AppState, ToastKind};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::{prelude::*, *};

struct SubscriptionForm {
    state: Entity<AppState>,
    server_id: i64,
    /// The subscription being edited (None: adding a new one).
    existing: Option<TopicSubscription>,
    topic: Entity<InputState>,
    qos: u8,
    color: Option<String>,
}

impl SubscriptionForm {
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let topic = input_value(&self.topic, cx);
        let (server_id, qos, color) = (self.server_id, self.qos, self.color.clone());
        let existing = self.existing.clone();
        let result = self.state.update(cx, |state, cx| match &existing {
            Some(sub) => state.update_subscription(sub, topic, qos, color, cx),
            None => state.add_subscription(server_id, topic, qos, color, cx),
        });
        match result {
            Ok(()) => {
                let message = if existing.is_some() {
                    tr!("subscription.updated")
                } else {
                    tr!("subscription.added")
                };
                notify(window, cx, ToastKind::Success, message);
                true
            }
            Err(err) => {
                notify(window, cx, ToastKind::Warning, err);
                false
            }
        }
    }

    fn swatch(&self, ix: usize, color: Option<String>, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let same = |a: &Option<String>, b: &Option<String>| match (a, b) {
            (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
            (None, None) => true,
            _ => false,
        };
        let selected = same(&self.color, &color);
        let fill = color.as_deref().and_then(color_from);
        let label = match &color {
            Some(c) => tr!("subscription.color_named", color = c),
            None => tr!("subscription.no_color"),
        };
        let entity = cx.entity();
        div()
            .id(("sub-color", ix))
            .role(Role::RadioButton)
            .aria_label(label.clone())
            .aria_selected(selected)
            .size(px(24.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(6.))
            .cursor_pointer()
            .border_2()
            .border_color(if selected { theme.foreground } else { theme.border })
            .when_some(fill, |this, fill| this.bg(fill))
            .when(fill.is_none(), |this| {
                this.child(Icon::new(IconName::Close).xsmall().text_color(theme.muted_foreground))
            })
            .tooltip(move |window, cx| Tooltip::new(label.clone()).build(window, cx))
            .on_click(move |_, _, cx| {
                entity.update(cx, |this, cx| {
                    this.color = color.clone();
                    cx.notify();
                })
            })
            .into_any_element()
    }
}

impl Render for SubscriptionForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        // A custom colour from the previous version's picker stays selectable.
        let mut colors: Vec<Option<String>> = std::iter::once(None)
            .chain(PRESET_COLORS.iter().map(|c| Some(c.to_string())))
            .collect();
        if let Some(custom) = &self.color
            && !PRESET_COLORS.iter().any(|c| c.eq_ignore_ascii_case(custom))
        {
            colors.push(Some(custom.clone()));
        }
        let mut swatches = Vec::with_capacity(colors.len());
        for (ix, color) in colors.into_iter().enumerate() {
            swatches.push(self.swatch(ix, color, cx));
        }
        v_flex()
            .gap_4()
            .child(form_row(
                "Topic",
                true,
                Input::new(&self.topic).aria_label(tr!("subscription.topic")),
                cx,
            ))
            .child(form_row(
                "QoS",
                false,
                segmented(
                    "sub-qos",
                    &tr!("subscription.qos"),
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
            .child(form_row(
                tr!("subscription.color"),
                false,
                h_flex().h(px(32.)).gap_1p5().children(swatches),
                cx,
            ))
            .child(
                div()
                    .pl(px(108.))
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(tr!("subscription.wildcard_hint")),
            )
    }
}

/// Add a subscription (`existing` = None) or edit one.
pub fn open(
    state: Entity<AppState>,
    server_id: i64,
    existing: Option<TopicSubscription>,
    window: &mut Window,
    cx: &mut App,
) {
    let initial_topic = existing.as_ref().map(|s| s.topic.clone()).unwrap_or_default();
    let topic = cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder(tr!("subscription.topic_placeholder"))
            .default_value(initial_topic)
    });
    let form = cx.new(|_| SubscriptionForm {
        state,
        server_id,
        qos: existing.as_ref().map(|s| s.qos.clamp(0, 2) as u8).unwrap_or(0),
        color: existing
            .as_ref()
            .and_then(|s| s.color.clone())
            .filter(|c| !c.trim().is_empty()),
        existing: existing.clone(),
        topic: topic.clone(),
    });
    let (title, ok) = if existing.is_some() {
        (tr!("subscription.edit_title"), tr!("common.save"))
    } else {
        (tr!("subscription.add_title"), tr!("subscription.subscribe"))
    };
    let first = std::cell::Cell::new(true);
    window.open_dialog(cx, move |dialog, window, cx| {
        if first.replace(false) {
            topic.update(cx, |s, cx| s.focus(window, cx));
        }
        let save_form = form.clone();
        dialog
            .title(title.clone())
            .w(px(520.))
            .child(form.clone())
            .footer(dialog_footer(ok.clone()))
            .on_ok(move |_, window, cx| save_form.update(cx, |form, cx| form.save(window, cx)))
    });
}
