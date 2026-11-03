//! Message detail dialog: full payload as Text / JSON / HEX, copy actions.

use super::common::{chip, notify, segmented};
use crate::backend::payload::{self, DetectedFormat};
use crate::state::{AppState, Direction, Message, PublishDraft, ToastKind};
use gpui_kit::component::button::Button;
use gpui_kit::component::dialog::DialogFooter;
use gpui_kit::component::input::{Editor, EditorState};
use gpui_kit::component::{ActiveTheme as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::{prelude::*, *};

/// Rendering limits of the detail view.
const TEXT_LIMIT: usize = 256 * 1024;
const JSON_LIMIT: usize = 1024 * 1024;
const HEX_LIMIT: usize = 64 * 1024;

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Text,
    Json,
    Hex,
}

struct MessageDetail {
    message: Message,
    view: View,
    content: Entity<EditorState>,
}

impl MessageDetail {
    fn new(message: Message, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let view = match message.format {
            DetectedFormat::Json => View::Json,
            DetectedFormat::Binary => View::Hex,
            DetectedFormat::Text => View::Text,
        };
        let content = cx.new(|cx| EditorState::new(window, cx).line_number(false).soft_wrap(true));
        let mut this = Self { message, view, content };
        this.set_view(view, window, cx);
        this
    }

    /// What the dialog shows. Large payloads are cut so the editor stays
    /// responsive; copying still yields the complete text (see [`Self::copy_content`]).
    fn render_content(&self) -> String {
        let m = &self.message;
        let total = m.payload.len();
        match self.view {
            View::Json if total <= JSON_LIMIT => {
                let text = m.full_text();
                payload::pretty_json(&text).unwrap_or(text)
            }
            View::Text | View::Json => {
                let (text, cut) = payload::text_prefix(&m.payload, TEXT_LIMIT);
                // The text view marks line breaks, so CR LF and trailing
                // newlines are visible.
                let text = if self.view == View::Text {
                    payload::mark_line_breaks(&text)
                } else {
                    text
                };
                if cut {
                    format!(
                        "{text}\n\n{}",
                        tr!("detail.text_truncated", kb = TEXT_LIMIT / 1024, total = total)
                    )
                } else {
                    text
                }
            }
            View::Hex => {
                let mut dump = payload::hex_dump(&m.payload[..total.min(HEX_LIMIT)])
                    .into_iter()
                    .map(|row| format!("{}  {}  {}", row.offset, row.hex, row.ascii))
                    .collect::<Vec<_>>()
                    .join("\n");
                if total > HEX_LIMIT {
                    dump.push_str(&format!(
                        "\n{}",
                        tr!("detail.hex_truncated", kb = HEX_LIMIT / 1024, total = total)
                    ));
                }
                dump
            }
        }
    }

    /// Clipboard content: the complete payload text (pretty-printed JSON when
    /// small enough, without line-break marks); the HEX view copies the whole
    /// payload as plain HEX (`01 02 FF`).
    fn copy_content(&self) -> String {
        let m = &self.message;
        match self.view {
            View::Hex => payload::to_hex_spaced(&m.payload),
            View::Json if m.payload.len() <= JSON_LIMIT => self.render_content(),
            _ => m.full_text(),
        }
    }

    fn set_view(&mut self, view: View, window: &mut Window, cx: &mut Context<Self>) {
        self.view = view;
        let text = self.render_content();
        self.content.update(cx, |state, cx| state.set_value(text, window, cx));
        cx.notify();
    }
}

impl Render for MessageDetail {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let m = &self.message;
        let entity = cx.entity();
        let view_ix = match self.view {
            View::Text => 0,
            View::Json => 1,
            View::Hex => 2,
        };
        let meta = |label: SharedString, value: SharedString| {
            h_flex()
                .gap_1()
                .text_xs()
                .child(div().text_color(theme.muted_foreground).child(label))
                .child(div().child(value))
        };

        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_x_4()
                    .gap_y_1()
                    .child(meta(
                        tr!("detail.direction"),
                        match m.direction {
                            Direction::Receive => tr!("detail.receive"),
                            Direction::Publish => tr!("detail.publish"),
                        },
                    ))
                    .child(meta(
                        tr!("detail.time"),
                        m.timestamp.format("%Y-%m-%d %H:%M:%S%.3f").to_string().into(),
                    ))
                    .child(meta("QoS".into(), m.qos.to_string().into()))
                    .child(meta(
                        "Retain".into(),
                        if m.retain { tr!("common.yes") } else { tr!("common.no") },
                    ))
                    .child(meta(tr!("detail.size"), tr!("detail.bytes", count = m.payload.len())))
                    .child(chip(m.format.label(), theme.primary, cx)),
            )
            .when_some(m.script_error.clone(), |this, err| {
                this.child(
                    div()
                        .p_2()
                        .rounded(theme.radius)
                        .bg(theme.danger.opacity(0.08))
                        .text_xs()
                        .text_color(theme.danger)
                        .child(tr!("detail.script_failed", error = err)),
                )
            })
            .child(segmented(
                "detail-view",
                &tr!("detail.view"),
                &[tr!("detail.text"), "JSON".into(), "HEX".into()],
                view_ix,
                move |ix, window, cx| {
                    entity.update(cx, |this, cx| {
                        let view = match ix {
                            1 => View::Json,
                            2 => View::Hex,
                            _ => View::Text,
                        };
                        this.set_view(view, window, cx)
                    })
                },
            ))
            .child(
                div().h(px(320.)).child(
                    Editor::new(&self.content)
                        .aria_label(tr!("detail.content"))
                        .h_full()
                        .readonly(true),
                ),
            )
    }
}

pub fn open(state: Entity<AppState>, message: Message, window: &mut Window, cx: &mut App) {
    let detail = cx.new(|cx| MessageDetail::new(message.clone(), window, cx));
    let title: SharedString = message.topic.clone();
    window.open_dialog(cx, move |dialog, _, _| {
        let copy_detail = detail.clone();
        let publish_detail = detail.clone();
        let state = state.clone();
        dialog.title(title.clone()).w(px(760.)).child(detail.clone()).footer(
            DialogFooter::new()
                .child(
                    Button::new("copy-payload")
                        .label(tr!("detail.copy_content"))
                        .icon(gpui_kit::component::IconName::Copy)
                        .on_click(move |_, window, cx| {
                            let text = copy_detail.read(cx).copy_content();
                            cx.write_to_clipboard(ClipboardItem::new_string(text));
                            notify(window, cx, ToastKind::Success, tr!("common.copied"));
                        }),
                )
                .child(
                    Button::new("copy-to-publish")
                        .label(tr!("detail.copy_to_publish"))
                        .icon(super::common::Lucide::Send)
                        .on_click(move |_, window, cx| {
                            let m = publish_detail.read(cx).message.clone();
                            let draft = PublishDraft {
                                topic: m.topic.to_string(),
                                payload: match m.format {
                                    DetectedFormat::Binary => payload::to_hex_spaced(&m.payload),
                                    _ => m.full_text(),
                                },
                                format: m.format.as_publish_format(),
                                qos: m.qos,
                                retain: m.retain,
                            };
                            state.update(cx, |state, cx| state.fill_publish(draft, cx));
                            window.close_dialog(cx);
                            notify(window, cx, ToastKind::Success, tr!("detail.copied_to_publish"));
                        }),
                )
                .child(super::common::close_button(tr!("common.close"), true)),
        )
    });
}
