//! Left sidebar: servers, subscriptions, theme toggle and settings.

use super::common::{IconName, Lucide, chip, confirm, status_dot};
use super::{server_dialog, settings_dialog, subscription_dialog};
use crate::backend::models::{MqttServer, Subscription as TopicSubscription};
use crate::state::AppState;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::component::{Disableable as _, StyledExt as _};
use gpui_kit::{prelude::*, *};

pub struct SidebarView {
    state: Entity<AppState>,
    _subscription: Subscription,
}

impl SidebarView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let subscription = cx.observe(&state, |_, _, cx| cx.notify());
        Self {
            state,
            _subscription: subscription,
        }
    }

    fn section_header(
        &self,
        id: &'static str,
        title: &'static str,
        enabled: bool,
        on_add: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
        cx: &App,
    ) -> impl IntoElement {
        h_flex()
            .flex_none()
            .justify_between()
            .px_4()
            .pt_3()
            .pb_2()
            .child(
                div()
                    .text_xs()
                    .font_semibold()
                    .text_color(cx.theme().muted_foreground)
                    .child(title),
            )
            .child(
                Button::new(id)
                    .primary()
                    .xsmall()
                    .rounded(px(999.))
                    .icon(IconName::Plus)
                    .disabled(!enabled)
                    .tooltip(if id == "add-server" {
                        "新增服务器"
                    } else {
                        "新增订阅"
                    })
                    .on_click(on_add),
            )
    }

    fn server_item(&self, server: &MqttServer, active: bool, cx: &App) -> impl IntoElement {
        let id = server.id.unwrap_or_default();
        let status = self.state.read(cx).status(id);
        let theme = cx.theme();
        let state = self.state.clone();
        let menu_state = self.state.clone();
        let name = server.name.clone();

        h_flex()
            .id(("server", id as u64))
            .mx_2()
            .px_3()
            .py_2()
            .gap_2()
            .rounded(theme.radius)
            .cursor_pointer()
            .when(active, |this| this.bg(theme.sidebar_accent))
            .when(!active, |this| this.hover(|s| s.bg(theme.sidebar_accent.opacity(0.5))))
            .on_click(move |_, _, cx| state.update(cx, |state, cx| state.set_active_server(id, cx)))
            .child(status_dot(&status, cx))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_sm()
                            .font_medium()
                            .truncate()
                            .text_color(if active {
                                theme.sidebar_accent_foreground
                            } else {
                                theme.sidebar_foreground
                            })
                            .child(server.name.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .truncate()
                            .text_color(theme.muted_foreground)
                            .child(server.address()),
                    ),
            )
            .child(
                Button::new(("server-menu", id as u64))
                    .ghost()
                    .xsmall()
                    .icon(IconName::EllipsisVertical)
                    .dropdown_menu(move |menu, _, _| {
                        let edit_state = menu_state.clone();
                        let dup_state = menu_state.clone();
                        let del_state = menu_state.clone();
                        let name = name.clone();
                        menu.item(
                            PopupMenuItem::new("编辑")
                                .icon(Lucide::SquarePen)
                                .on_click(move |_, window, cx| {
                                    let server = edit_state.read(cx).server(id).cloned();
                                    server_dialog::open(edit_state.clone(), server, window, cx);
                                }),
                        )
                        .item(
                            PopupMenuItem::new("复制")
                                .icon(IconName::Copy)
                                .on_click(move |_, _, cx| {
                                    dup_state.update(cx, |state, cx| {
                                        if let Err(err) = state.duplicate_server(id, cx) {
                                            state.toast(crate::state::ToastKind::Error, err, cx);
                                        }
                                    });
                                }),
                        )
                        .separator()
                        .item(
                            PopupMenuItem::new("删除")
                                .icon(Lucide::Trash)
                                .on_click(move |_, window, cx| {
                                    let del_state = del_state.clone();
                                    confirm(
                                        window,
                                        cx,
                                        "删除服务器",
                                        format!("确定要删除服务器「{name}」吗？其订阅、模板和脚本也会一并删除。"),
                                        "删除",
                                        move |_, cx| {
                                            del_state.update(cx, |state, cx| {
                                                if let Err(err) = state.delete_server(id, cx) {
                                                    state.toast(crate::state::ToastKind::Error, err, cx);
                                                }
                                            })
                                        },
                                    );
                                }),
                        )
                    }),
            )
    }

    fn subscription_item(&self, sub: &TopicSubscription, cx: &App) -> impl IntoElement {
        let id = sub.id.unwrap_or_default() as u64;
        let theme = cx.theme();
        let toggle_state = self.state.clone();
        let delete_state = self.state.clone();
        let toggle_sub = sub.clone();
        let delete_sub = sub.clone();
        let qos_color = if sub.is_active {
            theme.primary
        } else {
            theme.muted_foreground
        };

        h_flex()
            .id(("sub", id))
            .group("sub-item")
            .mx_2()
            .px_2()
            .py_1p5()
            .gap_2()
            .rounded(theme.radius)
            .hover(|s| s.bg(theme.sidebar_accent.opacity(0.5)))
            .child(
                Switch::new(("sub-switch", id))
                    .checked(sub.is_active)
                    .xsmall()
                    .on_click(move |checked, _, cx| {
                        let checked = *checked;
                        toggle_state.update(cx, |state, cx| state.toggle_subscription(&toggle_sub, checked, cx))
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_sm()
                    .font_family(theme.mono_font_family.clone())
                    .text_color(if sub.is_active {
                        theme.sidebar_foreground
                    } else {
                        theme.muted_foreground
                    })
                    .child(sub.topic.clone()),
            )
            .child(chip(format!("Q{}", sub.qos), qos_color, cx))
            .child(
                div().invisible().group_hover("sub-item", |s| s.visible()).child(
                    Button::new(("sub-delete", id))
                        .ghost()
                        .xsmall()
                        .icon(Lucide::Trash)
                        .tooltip("删除订阅")
                        .on_click(move |_, _, cx| {
                            delete_state.update(cx, |state, cx| state.remove_subscription(&delete_sub, cx))
                        }),
                ),
            )
    }
}

impl Render for SidebarView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let active_id = state.active_server_id;
        let servers = state.servers.clone();
        let subscriptions = active_id.map(|id| state.subscriptions(id)).unwrap_or_default();
        let is_dark = state.is_dark(cx);
        let theme = cx.theme().clone();

        let add_server_state = self.state.clone();
        let add_sub_state = self.state.clone();
        let theme_state = self.state.clone();
        let settings_state = self.state.clone();

        v_flex()
            .flex_none()
            .w(px(260.))
            .h_full()
            .bg(theme.sidebar)
            .border_r_1()
            .border_color(theme.sidebar_border)
            .child(
                h_flex()
                    .flex_none()
                    .h(px(60.))
                    .px_4()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.sidebar_border)
                    .child(Icon::new(Lucide::Layers).size(px(26.)).text_color(theme.primary))
                    .child(div().text_lg().font_bold().child("MQTT Client")),
            )
            .child(
                // Servers and subscriptions share one scroll area.
                v_flex()
                    .id("sidebar-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(self.section_header(
                        "add-server",
                        "SERVER",
                        true,
                        move |_, window, cx| server_dialog::open(add_server_state.clone(), None, window, cx),
                        cx,
                    ))
                    .child(
                        v_flex().gap_1().children(
                            servers
                                .iter()
                                .map(|server| self.server_item(server, server.id == active_id, cx)),
                        ),
                    )
                    .child(div().mx_4().my_3().h(px(1.)).bg(theme.sidebar_border))
                    .child(self.section_header(
                        "add-subscription",
                        "订阅",
                        active_id.is_some(),
                        move |_, window, cx| {
                            if let Some(server_id) = add_sub_state.read(cx).active_server_id {
                                subscription_dialog::open(add_sub_state.clone(), server_id, window, cx);
                            }
                        },
                        cx,
                    ))
                    .child(
                        v_flex()
                            .gap_0p5()
                            .pb_3()
                            .when(subscriptions.is_empty() && active_id.is_some(), |this| {
                                this.child(
                                    div()
                                        .px_4()
                                        .py_2()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child("暂无订阅，点击 + 添加"),
                                )
                            })
                            .children(subscriptions.iter().map(|sub| self.subscription_item(sub, cx))),
                    ),
            )
            .child(
                h_flex()
                    .flex_none()
                    .h(px(48.))
                    .px_2()
                    .justify_between()
                    .border_t_1()
                    .border_color(theme.sidebar_border)
                    .child(
                        Button::new("toggle-theme")
                            .ghost()
                            .small()
                            .icon(if is_dark { IconName::Sun } else { IconName::Moon })
                            .label(if is_dark { "浅色模式" } else { "深色模式" })
                            .on_click(move |_, _, cx| theme_state.update(cx, |state, cx| state.toggle_dark(cx))),
                    )
                    .child(
                        Button::new("open-settings")
                            .ghost()
                            .small()
                            .icon(IconName::Settings)
                            .tooltip("系统设置")
                            .on_click(move |_, window, cx| settings_dialog::open(settings_state.clone(), window, cx)),
                    ),
            )
    }
}
