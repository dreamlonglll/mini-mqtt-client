//! Root view: sidebar | (header, messages, publish panel, command bar).

use super::command_bar::CommandBar;
use super::common::{Lucide, empty_state, notify};
use super::header::HeaderView;
use super::message_list::MessageListView;
use super::publish_panel::PublishPanel;
use super::scheduled::Scheduler;
use super::server_dialog;
use super::sidebar::SidebarView;
use crate::state::{AppEvent, AppState};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::{prelude::*, *};

pub struct AppView {
    state: Entity<AppState>,
    sidebar: Entity<SidebarView>,
    header: Entity<HeaderView>,
    messages: Entity<MessageListView>,
    publish: Entity<PublishPanel>,
    command_bar: Entity<CommandBar>,
    _subscriptions: Vec<Subscription>,
}

impl AppView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let scheduler = cx.new(|cx| Scheduler::new(state.clone(), cx));
        let sidebar = cx.new(|cx| SidebarView::new(state.clone(), cx));
        let header = cx.new(|cx| HeaderView::new(state.clone(), cx));
        let messages = cx.new(|cx| MessageListView::new(state.clone(), window, cx));
        let publish = cx.new(|cx| PublishPanel::new(state.clone(), scheduler.clone(), window, cx));
        let command_bar = cx.new(|cx| CommandBar::new(state.clone(), cx));

        let subscriptions = vec![
            cx.observe(&state, |_, _, cx| cx.notify()),
            cx.subscribe_in(&state, window, |_, _, event: &AppEvent, window, cx| {
                if let AppEvent::Toast(kind, message) = event {
                    notify(window, cx, *kind, message.clone());
                }
            }),
            // "Follow system" keeps following when the OS switches theme.
            cx.observe_window_appearance(window, |this, window, cx| {
                this.state
                    .update(cx, |state, cx| state.system_appearance_changed(window, cx))
            }),
        ];

        Self {
            state,
            sidebar,
            header,
            messages,
            publish,
            command_bar,
            _subscriptions: subscriptions,
        }
    }
}

impl Render for AppView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let has_server = self.state.read(cx).active_server().is_some();
        let theme = cx.theme();
        let state = self.state.clone();

        let main = if has_server {
            v_flex()
                .flex_1()
                .min_h_0()
                .p_3()
                .gap_3()
                .child(div().flex_1().min_h_0().child(self.messages.clone()))
                .child(div().flex_none().child(self.publish.clone()))
                .into_any_element()
        } else {
            v_flex()
                .flex_1()
                .items_center()
                .justify_center()
                .gap_4()
                .child(
                    div()
                        .h(px(160.))
                        .w(px(360.))
                        .child(empty_state(Lucide::Server, tr!("app.no_server"), cx)),
                )
                .child(
                    Button::new("add-first-server")
                        .primary()
                        .icon(gpui_kit::component::IconName::Plus)
                        .label(tr!("app.add_server"))
                        .on_click(move |_, window, cx| server_dialog::open(state.clone(), None, window, cx)),
                )
                .into_any_element()
        };

        h_flex()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(self.sidebar.clone())
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .bg(theme.muted.opacity(0.35))
                    .when(has_server, |this| this.child(self.header.clone()))
                    .child(main)
                    .when(has_server, |this| this.child(self.command_bar.clone())),
            )
    }
}
