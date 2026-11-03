//! Small shared UI building blocks.

use crate::backend::mqtt::ConnectionStatus;
use crate::state::ToastKind;
use gpui_kit::component::StyledExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::{DialogClose, DialogFooter};
use gpui_kit::component::input::InputState;
use gpui_kit::component::notification::{Notification, NotificationType};
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
    group: &str,
    labels: &[impl AsRef<str>],
    selected: usize,
    on_select: impl Fn(usize, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let on_select = std::rc::Rc::new(on_select);
    h_flex()
        .gap_0p5()
        .children(labels.iter().enumerate().map(|(ix, label)| {
            let on_select = on_select.clone();
            let label = label.as_ref();
            let button = Button::new((id, ix))
                .small()
                .label(label.to_string())
                .accessibility_label(format!("{group} {label}"))
                .toggled(ix == selected);
            let button = if ix == selected {
                button.primary()
            } else {
                button.ghost()
            };
            button.on_click(move |_, window, cx| on_select(ix, window, cx))
        }))
}

/// Digit-only state for [`number_field`].
pub fn number_state(window: &mut Window, cx: &mut App, value: i64) -> Entity<InputState> {
    cx.new(|cx| {
        InputState::new(window, cx)
            .validate(|s, _| s.chars().all(|c| c.is_ascii_digit()))
            .default_value(value.to_string())
    })
}

fn step_number(state: &Entity<InputState>, delta: i64, min: i64, max: i64, window: &mut Window, cx: &mut App) {
    let current: i64 = state.read(cx).value().trim().parse().unwrap_or(min);
    let next = current.saturating_add(delta).clamp(min, max);
    state.update(cx, |s, cx| s.set_value(next.to_string(), window, cx));
}

/// Numeric field with −/+ buttons.
///
/// Used instead of GPUI Kit's `NumberInput`, whose wrapper and inner text
/// element both claim accessibility focus while focused (a debug assertion in
/// GPUI and a per-frame warning in release builds once a screen reader is on).
pub fn number_field(
    id: &'static str,
    label: &str,
    state: &Entity<InputState>,
    step: i64,
    min: i64,
    max: i64,
) -> impl IntoElement {
    let (dec, inc) = (state.clone(), state.clone());
    let label = SharedString::from(label.to_string());
    h_flex()
        .gap_1()
        .child(
            Button::new((id, 0usize))
                .outline()
                .icon(IconName::Minus)
                .accessibility_label(tr!("common.decrease", label = label))
                .on_click(move |_, window, cx| step_number(&dec, -step, min, max, window, cx)),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(56.))
                .child(gpui_kit::component::input::Input::new(state).aria_label(label.clone())),
        )
        .child(
            Button::new((id, 1usize))
                .outline()
                .icon(IconName::Plus)
                .accessibility_label(tr!("common.increase", label = label))
                .on_click(move |_, window, cx| step_number(&inc, step, min, max, window, cx)),
        )
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
    let type_ = match kind {
        ToastKind::Info => NotificationType::Info,
        ToastKind::Success => NotificationType::Success,
        ToastKind::Warning => NotificationType::Warning,
        ToastKind::Error => NotificationType::Error,
    };
    // Render the message ourselves so it carries an accessible name: screen
    // readers announce it with the alert (the stock message text has none).
    let note = Notification::new().with_type(type_).content(move |_, _, _| {
        div()
            .id("toast-message")
            .role(Role::Status)
            .aria_label(message.clone())
            .text_sm()
            .child(message.clone())
            .into_any_element()
    });
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
    alert(
        window,
        cx,
        true,
        title.into(),
        description.into(),
        ok_text.into(),
        on_ok,
    );
}

/// Question with a regular (primary) OK button, e.g. "go to the download page?".
pub fn ask(
    window: &mut Window,
    cx: &mut App,
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    ok_text: impl Into<SharedString>,
    on_ok: impl Fn(&mut Window, &mut App) + 'static,
) {
    alert(
        window,
        cx,
        false,
        title.into(),
        description.into(),
        ok_text.into(),
        on_ok,
    );
}

fn alert(
    window: &mut Window,
    cx: &mut App,
    danger: bool,
    title: SharedString,
    description: SharedString,
    ok_text: SharedString,
    on_ok: impl Fn(&mut Window, &mut App) + 'static,
) {
    let on_ok = std::rc::Rc::new(on_ok);
    window.open_alert_dialog(cx, move |alert, _, cx| {
        let on_ok = on_ok.clone();
        let (icon, color, variant) = if danger {
            (
                IconName::TriangleAlert,
                cx.theme().warning,
                gpui_kit::component::button::ButtonVariant::Danger,
            )
        } else {
            (
                IconName::Info,
                cx.theme().primary,
                gpui_kit::component::button::ButtonVariant::Primary,
            )
        };
        alert
            .icon(Icon::new(icon).text_color(color))
            .title(title.clone())
            .description(description.clone())
            .ok_text(ok_text.clone())
            .ok_variant(variant)
            .cancel_text(tr!("common.cancel"))
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
    DialogFooter::new()
        .child(close_button(tr!("common.cancel"), false))
        .child(
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
