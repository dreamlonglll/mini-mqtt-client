//! Bottom bar: quick commands (most used templates) and manager shortcuts.

use super::common::Lucide;
use super::{script_dialog, template_drawer};
use crate::state::AppState;
use gpui_kit::component::StyledExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Sizable as _, h_flex};
use gpui_kit::{prelude::*, *};

const QUICK_COMMANDS: usize = 8;

pub struct CommandBar {
    state: Entity<AppState>,
    _subscription: Subscription,
}

impl CommandBar {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let subscription = cx.observe(&state, |_, _, cx| cx.notify());
        Self {
            state,
            _subscription: subscription,
        }
    }
}

impl Render for CommandBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let state = self.state.read(cx);
        let server_id = state.active_server_id.unwrap_or_default();
        let templates = state.quick_templates(server_id, QUICK_COMMANDS);
        let templates_state = self.state.clone();
        let scripts_state = self.state.clone();

        h_flex()
            .flex_none()
            .h(px(48.))
            .px_4()
            .gap_2()
            .bg(theme.background)
            .border_t_1()
            .border_color(theme.border)
            .child(
                h_flex()
                    .flex_none()
                    .gap_1p5()
                    .text_sm()
                    .font_semibold()
                    .child(Icon::new(Lucide::Zap).small().text_color(theme.primary))
                    .child("快捷命令"),
            )
            .child(div().h(px(18.)).w(px(1.)).bg(theme.border))
            .child(
                h_flex()
                    .id("quick-commands")
                    .flex_1()
                    .min_w_0()
                    .gap_2()
                    .overflow_x_scroll()
                    .when(templates.is_empty(), |this| {
                        this.child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child("暂无模板，可在发布面板点击 ☆ 保存常用命令"),
                        )
                    })
                    .children(templates.into_iter().map(|t| {
                        let id = t.id.unwrap_or_default();
                        let state = self.state.clone();
                        Button::new(("quick", id as u64))
                            .outline()
                            .small()
                            .label(t.name.clone())
                            .tooltip(format!("{}  ·  使用 {} 次", t.topic, t.use_count))
                            .on_click(move |_, _, cx| state.update(cx, |state, cx| state.use_template(id, cx)))
                    })),
            )
            .child(
                Button::new("manage-templates")
                    .ghost()
                    .small()
                    .icon(IconName::FolderOpen)
                    .label("管理模板")
                    .on_click(move |_, window, cx| {
                        if let Some(id) = templates_state.read(cx).active_server_id {
                            template_drawer::open(templates_state.clone(), id, window, cx);
                        }
                    }),
            )
            .child(
                Button::new("manage-scripts")
                    .ghost()
                    .small()
                    .icon(Lucide::FileCode)
                    .label("管理脚本")
                    .on_click(move |_, window, cx| {
                        if let Some(id) = scripts_state.read(cx).active_server_id {
                            script_dialog::open(scripts_state.clone(), id, window, cx);
                        }
                    }),
            )
    }
}
