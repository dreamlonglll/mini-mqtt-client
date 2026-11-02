//! Live message stream of the active server.

use super::common::{IconName, Lucide, card, card_header, chip, confirm, empty_state, segmented};
use super::message_detail;
use crate::backend::payload::{self, DetectedFormat};
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
    _subscriptions: Vec<Subscription>,
}

impl MessageListView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("搜索 Topic / 内容..."));
        let subscriptions = vec![
            cx.subscribe(&state, |this, state, event: &AppEvent, cx| {
                // Messages of servers in the background do not touch this view.
                if let AppEvent::MessagesChanged(server_id) = event
                    && state.read(cx).active_server_id == Some(*server_id)
                {
                    this.refresh(cx);
                }
            }),
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
        direction_ok
            && (keyword.is_empty()
                || message.topic.to_lowercase().contains(keyword)
                || message.text.to_lowercase().contains(keyword))
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
            _ => payload::truncate(&message.text, 600).into(),
        };
        let state = self.state.clone();
        let detail = message.clone();

        div()
            .px_3()
            .pt_2()
            .child(
                v_flex()
                    .id(("message", message.id))
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
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_sm()
                                    .font_family(theme.mono_font_family.clone())
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
                                this.text_color(theme.muted_foreground).child("(空消息)")
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
                "没有匹配的消息"
            } else {
                "订阅 Topic 后，收到的消息将显示在这里"
            };
            div()
                .flex_1()
                .child(empty_state(IconName::Inbox, text, cx))
                .into_any_element()
        };

        card(cx)
            .size_full()
            .child(
                card_header(Lucide::MessageSquare, "消息列表", cx)
                    .child(chip(count_label, theme.muted_foreground, cx))
                    .child(div().flex_1())
                    .child(
                        div().w(px(220.)).child(
                            Input::new(&self.search).small().cleanable(true).prefix(
                                gpui_kit::component::Icon::new(IconName::Search)
                                    .small()
                                    .text_color(theme.muted_foreground),
                            ),
                        ),
                    )
                    .child(segmented(
                        "message-filter",
                        &["全部", "接收", "发布"],
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
                            .ghost()
                            .small()
                            .icon(Lucide::Trash)
                            .tooltip("清空消息")
                            .disabled(self.total == 0)
                            .on_click(move |_, window, cx| {
                                let clear_state = clear_state.clone();
                                confirm(
                                    window,
                                    cx,
                                    "确认清空",
                                    "确定要清空所有消息记录吗？",
                                    "清空",
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
