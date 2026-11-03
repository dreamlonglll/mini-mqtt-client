//! Left sidebar: servers, subscriptions, theme toggle and settings.

use super::common::{IconName, Lucide, ask, chip, color_from, confirm, status_dot};
use super::{server_dialog, settings_dialog, subscription_dialog};
use crate::backend::models::{MqttServer, Subscription as TopicSubscription};
use crate::backend::update;
use crate::state::AppState;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::component::{Disableable as _, StyledExt as _};
use gpui_kit::{prelude::*, *};

pub struct SidebarView {
    state: Entity<AppState>,
    /// Subscriptions of the active server, reloaded when the state changes.
    subscriptions: Vec<TopicSubscription>,
    /// Collapsed state of the server / subscription sections (not persisted).
    collapsed: [bool; 2],
    _subscription: Subscription,
}

const SERVERS: usize = 0;
const SUBSCRIPTIONS: usize = 1;

impl SidebarView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let subscription = cx.observe(&state, |this, state, cx| {
            this.subscriptions = Self::load(&state, cx);
            cx.notify();
        });
        Self {
            subscriptions: Self::load(&state, cx),
            collapsed: [false; 2],
            state,
            _subscription: subscription,
        }
    }

    fn load(state: &Entity<AppState>, cx: &App) -> Vec<TopicSubscription> {
        let state = state.read(cx);
        state
            .active_server_id
            .map(|id| state.subscriptions(id))
            .unwrap_or_default()
    }

    /// Title row of a sidebar section: click the title (or the arrow) to
    /// collapse / expand the list below it, `+` adds an item.
    fn section_header(
        &self,
        section: usize,
        id: &'static str,
        title: SharedString,
        enabled: bool,
        on_add: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let collapsed = self.collapsed[section];
        let toggle = {
            let entity = cx.entity();
            move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                entity.update(cx, |this, cx| {
                    this.collapsed[section] = !this.collapsed[section];
                    cx.notify();
                })
            }
        };
        let toggle_label = if collapsed {
            tr!("sidebar.expand_section", section = title)
        } else {
            tr!("sidebar.collapse_section", section = title)
        };
        h_flex()
            .flex_none()
            .justify_between()
            .pl_2()
            .pr_4()
            .pt_3()
            .pb_2()
            .child(
                h_flex()
                    .gap_0p5()
                    .child(
                        Button::new(("toggle-section", section))
                            .accessibility_label(toggle_label.clone())
                            .ghost()
                            .xsmall()
                            .icon(if collapsed {
                                IconName::ChevronRight
                            } else {
                                IconName::ChevronDown
                            })
                            .tooltip(toggle_label)
                            .on_click(toggle.clone()),
                    )
                    .child(
                        div()
                            .id(("section-title", section))
                            .cursor_pointer()
                            .text_xs()
                            .font_semibold()
                            .text_color(cx.theme().muted_foreground)
                            .on_click(toggle)
                            .child(title.clone()),
                    ),
            )
            .child({
                let label = if id == "add-server" {
                    tr!("sidebar.add_server")
                } else {
                    tr!("sidebar.add_subscription")
                };
                Button::new(id)
                    .accessibility_label(label.clone())
                    .primary()
                    .xsmall()
                    .rounded(px(999.))
                    .icon(IconName::Plus)
                    .disabled(!enabled)
                    .tooltip(label)
                    .on_click(on_add)
            })
    }

    /// `v2.0.0` next to the title; with a newer release it becomes a button
    /// (with a red dot) that offers to open the download page.
    fn version_badge(&self, cx: &App) -> AnyElement {
        let theme = cx.theme();
        let current = format!("v{}", update::current_version());
        let Some(latest) = self.state.read(cx).available_update().map(str::to_string) else {
            return div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(current)
                .into_any_element();
        };
        let hint = tr!("update.badge_hint", version = latest);
        h_flex()
            .gap_0p5()
            .child(
                Button::new("app-version")
                    .ghost()
                    .xsmall()
                    .label(current)
                    .text_color(theme.primary)
                    .accessibility_label(hint.clone())
                    .tooltip(hint)
                    .on_click(move |_, window, cx| {
                        ask(
                            window,
                            cx,
                            tr!("update.found_title"),
                            tr!("update.found_confirm", version = latest),
                            tr!("update.go_download"),
                            |_, cx| cx.open_url(update::RELEASES_PAGE),
                        )
                    }),
            )
            .child(div().size(px(8.)).rounded_full().bg(theme.danger))
            .into_any_element()
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
            .role(Role::ListItem)
            .aria_label(format!(
                "{} {} {} {}{}",
                tr!("sidebar.server"),
                server.name,
                server.address(),
                status.label(),
                if active {
                    format!(" {}", tr!("sidebar.current"))
                } else {
                    String::new()
                }
            ))
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
                    .accessibility_label(tr!("sidebar.server_menu", name = server.name))
                    .ghost()
                    .xsmall()
                    .icon(IconName::EllipsisVertical)
                    .dropdown_menu(move |menu, _, _| {
                        let edit_state = menu_state.clone();
                        let dup_state = menu_state.clone();
                        let del_state = menu_state.clone();
                        let name = name.clone();
                        menu.item(PopupMenuItem::new(tr!("common.edit")).icon(Lucide::SquarePen).on_click(
                            move |_, window, cx| {
                                let server = edit_state.read(cx).server(id).cloned();
                                server_dialog::open(edit_state.clone(), server, window, cx);
                            },
                        ))
                        .item(
                            PopupMenuItem::new(tr!("common.duplicate"))
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
                            PopupMenuItem::new(tr!("common.delete")).icon(Lucide::Trash).on_click(
                                move |_, window, cx| {
                                    let del_state = del_state.clone();
                                    confirm(
                                        window,
                                        cx,
                                        tr!("sidebar.delete_server_title"),
                                        tr!("sidebar.delete_server_confirm", name = name),
                                        tr!("common.delete"),
                                        move |_, cx| {
                                            del_state.update(cx, |state, cx| {
                                                if let Err(err) = state.delete_server(id, cx) {
                                                    state.toast(crate::state::ToastKind::Error, err, cx);
                                                }
                                            })
                                        },
                                    );
                                },
                            ),
                        )
                    }),
            )
    }

    fn subscription_item(&self, sub: &TopicSubscription, cx: &App) -> impl IntoElement {
        let id = sub.id.unwrap_or_default() as u64;
        let theme = cx.theme();
        let toggle_state = self.state.clone();
        let delete_state = self.state.clone();
        let edit_state = self.state.clone();
        let toggle_sub = sub.clone();
        let delete_sub = sub.clone();
        let edit_sub = sub.clone();
        let mark = sub.color.as_deref().and_then(color_from);
        let full_topic: SharedString = sub.topic.clone().into();
        let qos_color = if sub.is_active {
            theme.primary
        } else {
            theme.muted_foreground
        };

        h_flex()
            .id(("sub", id))
            .role(Role::ListItem)
            .aria_label(format!(
                "{} {} Q{} {}",
                tr!("sidebar.subscription"),
                sub.topic,
                sub.qos,
                if sub.is_active {
                    tr!("common.enabled")
                } else {
                    tr!("common.disabled")
                }
            ))
            .group("sub-item")
            .mx_2()
            .px_2()
            .py_1p5()
            .gap_2()
            .rounded(theme.radius)
            .hover(|s| s.bg(theme.sidebar_accent.opacity(0.5)))
            .child(
                Switch::new(("sub-switch", id))
                    .accessibility_label(tr!("sidebar.enable_subscription", topic = sub.topic))
                    .checked(sub.is_active)
                    .xsmall()
                    .on_click(move |checked, _, cx| {
                        let checked = *checked;
                        toggle_state.update(cx, |state, cx| state.toggle_subscription(&toggle_sub, checked, cx))
                    }),
            )
            .when_some(mark, |this, mark| {
                this.child(div().flex_none().size(px(8.)).rounded_full().bg(mark))
            })
            .child(
                div()
                    .id(("sub-topic", id))
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
                    // Long topics are cut off; hovering shows the whole one.
                    .tooltip(move |window, cx| Tooltip::new(full_topic.clone()).build(window, cx))
                    .child(sub.topic.clone()),
            )
            .child(chip(format!("Q{}", sub.qos), qos_color, cx))
            .child(
                h_flex()
                    .invisible()
                    .group_hover("sub-item", |s| s.visible())
                    .child(
                        Button::new(("sub-edit", id))
                            .accessibility_label(tr!("sidebar.edit_subscription_of", topic = sub.topic))
                            .ghost()
                            .xsmall()
                            .icon(Lucide::SquarePen)
                            .tooltip(tr!("subscription.edit_title"))
                            .on_click(move |_, window, cx| {
                                subscription_dialog::open(
                                    edit_state.clone(),
                                    edit_sub.server_id,
                                    Some(edit_sub.clone()),
                                    window,
                                    cx,
                                )
                            }),
                    )
                    .child(
                        Button::new(("sub-delete", id))
                            .accessibility_label(tr!("sidebar.delete_subscription_of", topic = sub.topic))
                            .ghost()
                            .xsmall()
                            .icon(Lucide::Trash)
                            .tooltip(tr!("sidebar.delete_subscription"))
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
        let subscriptions = self.subscriptions.clone();
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
                    .child(div().text_lg().font_bold().child("MQTT Client"))
                    .child(self.version_badge(cx)),
            )
            .child(
                // Servers and subscriptions share one scroll area.
                v_flex()
                    .id("sidebar-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(self.section_header(
                        SERVERS,
                        "add-server",
                        tr!("sidebar.servers"),
                        true,
                        move |_, window, cx| server_dialog::open(add_server_state.clone(), None, window, cx),
                        cx,
                    ))
                    .when(!self.collapsed[SERVERS], |this| {
                        this.child(
                            v_flex().gap_1().children(
                                servers
                                    .iter()
                                    .map(|server| self.server_item(server, server.id == active_id, cx)),
                            ),
                        )
                    })
                    .child(div().mx_4().my_3().h(px(1.)).bg(theme.sidebar_border))
                    .child(self.section_header(
                        SUBSCRIPTIONS,
                        "add-subscription",
                        tr!("sidebar.subscriptions"),
                        active_id.is_some(),
                        move |_, window, cx| {
                            if let Some(server_id) = add_sub_state.read(cx).active_server_id {
                                subscription_dialog::open(add_sub_state.clone(), server_id, None, window, cx);
                            }
                        },
                        cx,
                    ))
                    .when(!self.collapsed[SUBSCRIPTIONS], |this| {
                        this.child(
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
                                            .child(tr!("sidebar.no_subscriptions")),
                                    )
                                })
                                .children(subscriptions.iter().map(|sub| self.subscription_item(sub, cx))),
                        )
                    }),
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
                            .label(if is_dark {
                                tr!("sidebar.light_mode")
                            } else {
                                tr!("sidebar.dark_mode")
                            })
                            .on_click(move |_, _, cx| theme_state.update(cx, |state, cx| state.toggle_dark(cx))),
                    )
                    .child(
                        Button::new("open-settings")
                            .accessibility_label(tr!("settings.title"))
                            .ghost()
                            .small()
                            .icon(IconName::Settings)
                            .tooltip(tr!("settings.title"))
                            .on_click(move |_, window, cx| settings_dialog::open(settings_state.clone(), window, cx)),
                    ),
            )
    }
}
