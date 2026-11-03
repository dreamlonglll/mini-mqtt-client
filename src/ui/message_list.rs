//! Live message stream of the active server.

use super::common::{IconName, Lucide, card, card_header, chip, color_from, confirm, empty_state, segmented};
use super::message_detail;
use crate::backend::payload::{self, DetectedFormat};
use crate::backend::topic;
use crate::state::{AppEvent, AppState, Direction, Message};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::component::{Disableable as _, StyledExt as _};
use gpui_kit::{prelude::*, *};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Filter {
    All,
    Receive,
    Publish,
}

pub struct MessageListView {
    state: Entity<AppState>,
    search: Entity<InputState>,
    filter: Filter,
    list_state: ListState,
    /// Filtered snapshot, newest first.
    visible: Vec<Message>,
    total: usize,
    server_id: Option<i64>,
    /// Colour marks of the active server's subscriptions: `(filter, colour)`.
    marks: Vec<(String, Hsla)>,
    _subscriptions: Vec<Subscription>,
}

impl MessageListView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder(tr!("messages.search_placeholder")));
        let subscriptions = vec![
            cx.subscribe_in(
                &state,
                window,
                |this, state, event: &AppEvent, window, cx| match event {
                    // Messages of servers in the background do not touch this view.
                    AppEvent::MessagesChanged(server_id) if state.read(cx).active_server_id == Some(*server_id) => {
                        this.refresh(cx);
                    }
                    AppEvent::LanguageChanged => this.search.update(cx, |s, cx| {
                        s.set_placeholder(tr!("messages.search_placeholder"), window, cx)
                    }),
                    _ => {}
                },
            ),
            cx.subscribe(&search, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.refresh(cx);
                }
            }),
        ];
        let mut this = Self {
            state,
            search,
            filter: Filter::All,
            list_state: ListState::new(0, ListAlignment::Top, px(400.)),
            visible: Vec::new(),
            total: 0,
            server_id: None,
            marks: Vec::new(),
            _subscriptions: subscriptions,
        };
        this.refresh(cx);
        this
    }

    fn matches(&self, message: &Message, keyword: &str) -> bool {
        let direction_ok = match self.filter {
            Filter::All => true,
            Filter::Receive => message.direction == Direction::Receive,
            Filter::Publish => message.direction == Direction::Publish,
        };
        direction_ok && (keyword.is_empty() || message.search_text.contains(keyword))
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let keyword = self.search.read(cx).value().trim().to_lowercase();
        let state = self.state.read(cx);
        let server_id = state.active_server_id;
        let all = server_id.and_then(|id| state.messages(id));
        let next: Vec<Message> = all
            .map(|list| list.iter().filter(|m| self.matches(m, &keyword)).cloned().collect())
            .unwrap_or_default();
        self.total = all.map(|l| l.len()).unwrap_or(0);

        // New messages arrive at the front. Splice instead of resetting so
        // the scroll position of someone reading older messages is kept.
        let old_first = self.visible.first().map(|m| m.id);
        let same_server = server_id == self.server_id;
        let prepended = match old_first {
            Some(first) if same_server => next.iter().position(|m| m.id == first),
            _ => None,
        };
        match prepended {
            Some(added) if next.len() >= added && self.visible.len() + added >= next.len() => {
                let top = self.list_state.logical_scroll_top();
                let at_top = top.item_ix == 0 && top.offset_in_item <= px(1.);
                let kept = next.len() - added;
                let removed = self.visible.len() - kept;
                if removed > 0 {
                    self.list_state.splice(kept..kept + removed, 0);
                }
                if added > 0 {
                    self.list_state.splice(0..0, added);
                    // Follow the newest message unless the user scrolled away.
                    if at_top {
                        self.list_state.scroll_to(ListOffset {
                            item_ix: 0,
                            offset_in_item: px(0.),
                        });
                    }
                }
            }
            _ => self.list_state.reset(next.len()),
        }
        self.visible = next;
        self.server_id = server_id;
        cx.notify();
    }

    /// Colour of the subscription a received message came through: an exact
    /// filter first, then the first matching wildcard filter.
    fn mark_for(&self, message: &Message) -> Option<Hsla> {
        if message.direction != Direction::Receive {
            return None;
        }
        let topic = message.topic.as_ref();
        self.marks
            .iter()
            .find(|(filter, _)| filter == topic)
            .or_else(|| {
                self.marks
                    .iter()
                    .find(|(filter, _)| topic::matches_filter(filter, topic))
            })
            .map(|(_, color)| *color)
    }

    fn render_message(&self, ix: usize, cx: &App) -> AnyElement {
        let Some(message) = self.visible.get(ix).cloned() else {
            return div().into_any_element();
        };
        let theme = cx.theme();
        let (accent, dir_label, dir_icon) = match message.direction {
            Direction::Receive => (theme.success, "RCV", IconName::ArrowDown),
            Direction::Publish => (theme.primary, "PUB", IconName::ArrowUp),
        };
        let format_color = match message.format {
            DetectedFormat::Json => theme.success,
            DetectedFormat::Binary => theme.warning,
            DetectedFormat::Text => theme.muted_foreground,
        };
        let preview: SharedString = match message.format {
            DetectedFormat::Binary => {
                let shown = &message.payload[..message.payload.len().min(48)];
                let mut hex = payload::to_hex_spaced(shown);
                if message.payload.len() > 48 {
                    hex.push_str(" …");
                }
                hex.into()
            }
            _ => payload::truncate(&message.preview, 600).into(),
        };
        let state = self.state.clone();
        let mark = self.mark_for(&message);
        let detail = message.clone();

        div()
            .px_3()
            .pt_2()
            .child(
                v_flex()
                    .id(("message", message.id))
                    .role(Role::ListItem)
                    .aria_label(format!(
                        "{} {} {} {} Q{}{} {}{}",
                        tr!("messages.item"),
                        dir_label,
                        message.topic,
                        message.format.label(),
                        message.qos,
                        if message.retain { " R" } else { "" },
                        payload::truncate(&preview, 300),
                        message
                            .script_error
                            .as_ref()
                            .map(|e| format!(" {}: {e}", tr!("messages.script_error")))
                            .unwrap_or_default()
                    ))
                    .gap_1p5()
                    .p_2p5()
                    .pl_3()
                    .relative()
                    .overflow_hidden()
                    .rounded(theme.radius)
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.background)
                    .child(div().absolute().left_0().top_0().bottom_0().w(px(3.)).bg(accent))
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.list_hover))
                    .on_click(move |_, window, cx| message_detail::open(state.clone(), detail.clone(), window, cx))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                h_flex()
                                    .flex_none()
                                    .gap_0p5()
                                    .px_1p5()
                                    .rounded(px(4.))
                                    .bg(accent.opacity(0.12))
                                    .text_color(accent)
                                    .text_xs()
                                    .font_semibold()
                                    .child(Icon::new(dir_icon).xsmall())
                                    .child(dir_label),
                            )
                            .when_some(mark, |this, mark| {
                                this.child(div().flex_none().size(px(8.)).rounded_full().bg(mark))
                            })
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_sm()
                                    .font_family(theme.mono_font_family.clone())
                                    .when_some(mark, |this, mark| this.text_color(mark))
                                    .child(message.topic.clone()),
                            )
                            .child(chip(message.format.label(), format_color, cx))
                            .child(chip(format!("Q{}", message.qos), theme.primary, cx))
                            .when(message.retain, |this| this.child(chip("R", theme.warning, cx)))
                            .child(
                                div()
                                    .flex_none()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(message.time_label()),
                            ),
                    )
                    .child(
                        div()
                            .px_2()
                            .py_1p5()
                            .rounded(px(4.))
                            .bg(theme.muted.opacity(0.6))
                            .text_xs()
                            .font_family(theme.mono_font_family.clone())
                            .line_clamp(6)
                            .when(preview.is_empty(), |this| {
                                this.text_color(theme.muted_foreground)
                                    .child(tr!("messages.empty_payload"))
                            })
                            .when(!preview.is_empty(), |this| this.child(preview)),
                    )
                    .when_some(message.script_error.clone(), |this, error| {
                        this.child(
                            h_flex()
                                .gap_1()
                                .text_xs()
                                .text_color(theme.danger)
                                .child(Icon::new(IconName::TriangleAlert).xsmall())
                                .child(div().truncate().child(error)),
                        )
                    }),
            )
            .into_any_element()
    }
}

impl Render for MessageListView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.marks = {
            let state = self.state.read(cx);
            state
                .active_server_id
                .map(|id| state.subscriptions(id))
                .unwrap_or_default()
                .into_iter()
                .filter_map(|s| Some((s.topic, s.color.as_deref().and_then(color_from)?)))
                .collect()
        };
        let theme = cx.theme().clone();
        let filter_ix = match self.filter {
            Filter::All => 0,
            Filter::Receive => 1,
            Filter::Publish => 2,
        };
        let entity = cx.entity();
        let clear_state = self.state.clone();
        let has_messages = !self.visible.is_empty();
        let count_label = if self.visible.len() == self.total {
            self.total.to_string()
        } else {
            format!("{}/{}", self.visible.len(), self.total)
        };

        let list = if has_messages {
            let view = cx.entity().downgrade();
            div()
                .relative()
                .flex_1()
                .min_h_0()
                .child(
                    list(self.list_state.clone(), move |ix, _window, cx| {
                        view.upgrade()
                            .map(|view| view.read(cx).render_message(ix, cx))
                            .unwrap_or_else(|| div().into_any_element())
                    })
                    .size_full()
                    .pb_2(),
                )
                .vertical_scrollbar(&self.list_state)
                .into_any_element()
        } else {
            let text = if self.total > 0 {
                tr!("messages.no_match")
            } else {
                tr!("messages.empty")
            };
            div()
                .flex_1()
                .child(empty_state(IconName::Inbox, text, cx))
                .into_any_element()
        };

        card(cx)
            .size_full()
            .child(
                card_header(Lucide::MessageSquare, tr!("messages.title"), cx)
                    .child(
                        chip(count_label.clone(), theme.muted_foreground, cx)
                            .id("message-count")
                            .role(Role::Status)
                            .aria_label(format!("{} {count_label}", tr!("messages.count"))),
                    )
                    .child(div().flex_1())
                    .child(
                        div().w(px(220.)).child(
                            Input::new(&self.search)
                                .aria_label(tr!("messages.search"))
                                .small()
                                .cleanable(true)
                                .prefix(
                                    gpui_kit::component::Icon::new(IconName::Search)
                                        .small()
                                        .text_color(theme.muted_foreground),
                                ),
                        ),
                    )
                    .child(segmented(
                        "message-filter",
                        &tr!("messages.filter"),
                        &[tr!("messages.all"), tr!("messages.received"), tr!("messages.published")],
                        filter_ix,
                        move |ix, _, cx| {
                            entity.update(cx, |this, cx| {
                                this.filter = match ix {
                                    1 => Filter::Receive,
                                    2 => Filter::Publish,
                                    _ => Filter::All,
                                };
                                this.refresh(cx);
                            })
                        },
                    ))
                    .child(
                        Button::new("clear-messages")
                            .accessibility_label(tr!("messages.clear"))
                            .ghost()
                            .small()
                            .icon(Lucide::Trash)
                            .tooltip(tr!("messages.clear"))
                            .disabled(self.total == 0)
                            .on_click(move |_, window, cx| {
                                let clear_state = clear_state.clone();
                                confirm(
                                    window,
                                    cx,
                                    tr!("messages.clear_title"),
                                    tr!("messages.clear_confirm"),
                                    tr!("common.clear"),
                                    move |_, cx| {
                                        clear_state.update(cx, |state, cx| {
                                            if let Some(id) = state.active_server_id {
                                                state.clear_messages(id, cx);
                                            }
                                        })
                                    },
                                );
                            }),
                    ),
            )
            .child(list)
    }
}
