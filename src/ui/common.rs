//! Small shared UI building blocks.

use crate::backend::mqtt::ConnectionStatus;
use crate::state::ToastKind;
use gpui_kit::component::StyledExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::{DialogClose, DialogFooter};
use gpui_kit::component::input::InputState;
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::{prelude::*, *};

pub use gpui_kit::assets::IconName as Lucide;
pub use gpui_kit::component::IconName;

pub fn status_color(status: &ConnectionStatus, cx: &App) -> Hsla {
    let theme = cx.theme();
    match status {
        ConnectionStatus::Connected => theme.success,
        ConnectionStatus::Connecting => theme.warning,
        ConnectionStatus::Error(_) => theme.danger,
        ConnectionStatus::Disconnected => theme.muted_foreground.opacity(0.6),
    }
}

pub fn status_dot(status: &ConnectionStatus, cx: &App) -> Div {
    div()
        .flex_none()
        .size(px(8.))
        .rounded_full()
        .bg(status_color(status, cx))
}

/// A bordered panel, like the Element Plus cards of the previous UI.
pub fn card(cx: &App) -> Div {
    v_flex()
        .bg(cx.theme().background)
        .border_1()
        .border_color(cx.theme().border)
        .rounded(cx.theme().radius_lg)
        .overflow_hidden()
}

pub fn card_header(icon: impl Into<Icon>, title: impl Into<SharedString>, cx: &App) -> Div {
    h_flex()
        .flex_none()
        .h(px(48.))
        .px_4()
        .gap_2()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(Icon::new(icon).small().text_color(cx.theme().muted_foreground))
        .child(div().font_semibold().child(title.into()))
}

/// Small rounded label, used for QoS / format / retain markers.
pub fn chip(text: impl Into<SharedString>, color: Hsla, cx: &App) -> Div {
    let _ = cx;
    div()
        .flex_none()
        .px_1p5()
        .py(px(1.))
        .rounded(px(4.))
        .border_1()
        .border_color(color.opacity(0.35))
        .bg(color.opacity(0.08))
        .text_color(color)
        .text_xs()
        .child(text.into())
}

/// `label` column + control, used by all forms.
pub fn form_row(label: impl Into<SharedString>, required: bool, child: impl IntoElement, cx: &App) -> Div {
    h_flex()
        .w_full()
        .gap_3()
        .items_start()
        .child(
            h_flex()
                .flex_none()
                .w(px(96.))
                .h(px(32.))
                .justify_end()
                .gap_0p5()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .when(required, |this| {
                    this.child(div().text_color(cx.theme().danger).child("*"))
                })
                .child(label.into()),
        )
        .child(div().flex_1().min_w_0().child(child))
}

pub fn section_label(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_sm()
        .font_semibold()
        .pb_1()
        .mb_1()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(text.into())
}

/// A row of toggle buttons where exactly one is selected.
pub fn segmented(
    id: &'static str,
    labels: &[&str],
    selected: usize,
    on_select: impl Fn(usize, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let on_select = std::rc::Rc::new(on_select);
    h_flex()
        .gap_0p5()
        .children(labels.iter().enumerate().map(|(ix, label)| {
            let on_select = on_select.clone();
            let button = Button::new((id, ix)).small().label(label.to_string());
            let button = if ix == selected {
                button.primary()
            } else {
                button.ghost()
            };
            button.on_click(move |_, window, cx| on_select(ix, window, cx))
        }))
}

pub fn input_value(state: &Entity<InputState>, cx: &App) -> String {
    state.read(cx).value().to_string()
}

pub fn opt_value(state: &Entity<InputState>, cx: &App) -> Option<String> {
    let v = input_value(state, cx);
    let v = v.trim();
    (!v.is_empty()).then(|| v.to_string())
}

pub fn notify(window: &mut Window, cx: &mut App, kind: ToastKind, message: impl Into<SharedString>) {
    let message = message.into();
    let note = match kind {
        ToastKind::Info => Notification::info(message),
        ToastKind::Success => Notification::success(message),
        ToastKind::Warning => Notification::warning(message),
        ToastKind::Error => Notification::error(message),
    };
    window.push_notification(note, cx);
}

/// Confirmation dialog with a destructive OK button.
pub fn confirm(
    window: &mut Window,
    cx: &mut App,
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    ok_text: impl Into<SharedString>,
    on_ok: impl Fn(&mut Window, &mut App) + 'static,
) {
    let title = title.into();
    let description = description.into();
    let ok_text = ok_text.into();
    let on_ok = std::rc::Rc::new(on_ok);
    window.open_alert_dialog(cx, move |alert, _, cx| {
        let on_ok = on_ok.clone();
        alert
            .icon(Icon::new(IconName::TriangleAlert).text_color(cx.theme().warning))
            .title(title.clone())
            .description(description.clone())
            .ok_text(ok_text.clone())
            .ok_variant(gpui_kit::component::button::ButtonVariant::Danger)
            .cancel_text("取消")
            .show_cancel(true)
            .on_ok(move |_, window, cx| {
                on_ok(window, cx);
                true
            })
    });
}

/// Footer button that closes the dialog (compact, right-aligned).
pub fn close_button(label: impl Into<SharedString>, primary: bool) -> impl IntoElement {
    let label = label.into();
    div().min_w(px(88.)).child(DialogClose::new().trigger(move |button| {
        let button = button.label(label);
        if primary { button.primary() } else { button }
    }))
}

/// Standard dialog footer: Cancel + a primary action that triggers `on_ok`.
pub fn dialog_footer(ok_label: impl Into<SharedString>) -> DialogFooter {
    let ok_label = ok_label.into();
    DialogFooter::new().child(close_button("取消", false)).child(
        div().min_w(px(88.)).child(
            gpui_kit::component::dialog::DialogAction::new()
                .child(Button::new("dialog-ok").primary().px_4().label(ok_label)),
        ),
    )
}

pub fn empty_state(icon: impl Into<Icon>, text: impl Into<SharedString>, cx: &App) -> Div {
    v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .gap_3()
        .text_color(cx.theme().muted_foreground)
        .child(
            Icon::new(icon)
                .size(px(48.))
                .text_color(cx.theme().muted_foreground.opacity(0.5)),
        )
        .child(div().text_sm().child(text.into()))
}
