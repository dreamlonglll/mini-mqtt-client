//! Environment variable manager (right-side sheet) and its edit dialog.

use super::common::{IconName, Lucide, confirm, dialog_footer, empty_state, form_row, input_value, notify, opt_value};
use crate::backend::models::EnvVariable;
use crate::state::{AppState, ToastKind};
use gpui_kit::component::StyledExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, Icon, Placement, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::{prelude::*, *};

fn usage(name: &str) -> String {
    format!("{{{{{name}}}}}")
}

fn copy(text: String, window: &mut Window, cx: &mut App) {
    cx.write_to_clipboard(ClipboardItem::new_string(text));
    notify(window, cx, ToastKind::Success, "已复制到剪贴板");
}

struct EnvBrowser {
    state: Entity<AppState>,
    server_id: i64,
    search: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

impl EnvBrowser {
    fn new(state: Entity<AppState>, server_id: i64, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("搜索变量名或描述"));
        let subscriptions = vec![
            cx.observe(&state, |_, _, cx| cx.notify()),
            cx.subscribe(&search, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }),
        ];
        Self {
            state,
            server_id,
            search,
            _subscriptions: subscriptions,
        }
    }

    fn render_card(&self, var: &EnvVariable, cx: &App) -> impl IntoElement {
        let theme = cx.theme();
        let id = var.id.unwrap_or_default();
        let menu_state = self.state.clone();
        let menu_var = var.clone();
        let usage_text = usage(&var.name);

        v_flex()
            .id(("env-card", id as u64))
            .role(Role::ListItem)
            .aria_label(format!("环境变量 {} {}", var.name, var.value))
            .p_3()
            .gap_2()
            .rounded(theme.radius_lg)
            .border_1()
            .border_color(theme.border)
            .bg(theme.background)
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_semibold()
                            .font_family(theme.mono_font_family.clone())
                            .child(var.name.clone()),
                    )
                    .child(
                        Button::new(("env-menu", id as u64))
                            .accessibility_label(format!("变量菜单 {}", var.name))
                            .ghost()
                            .xsmall()
                            .icon(IconName::Ellipsis)
                            .dropdown_menu(move |menu, _, _| {
                                let (edit_state, del_state) = (menu_state.clone(), menu_state.clone());
                                let (edit_var, value, name) =
                                    (menu_var.clone(), menu_var.value.clone(), menu_var.name.clone());
                                menu.item(PopupMenuItem::new("编辑").icon(Lucide::SquarePen).on_click(
                                    move |_, window, cx| {
                                        open_dialog(
                                            edit_state.clone(),
                                            edit_var.server_id,
                                            Some(edit_var.clone()),
                                            window,
                                            cx,
                                        )
                                    },
                                ))
                                .item(
                                    PopupMenuItem::new("复制值")
                                        .icon(IconName::Copy)
                                        .on_click(move |_, window, cx| copy(value.clone(), window, cx)),
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
                                                "删除变量",
                                                format!("确定要删除变量「{name}」吗？"),
                                                "删除",
                                                move |window, cx| match del_state
                                                    .update(cx, |state, cx| state.delete_env_variable(id, cx))
                                                {
                                                    Ok(()) => notify(window, cx, ToastKind::Success, "变量已删除"),
                                                    Err(err) => notify(window, cx, ToastKind::Error, err),
                                                },
                                            );
                                        }),
                                )
                            }),
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
                    .line_clamp(4)
                    .child(var.value.clone()),
            )
            .when_some(var.description.clone().filter(|d| !d.is_empty()), |this, desc| {
                this.child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .line_clamp(2)
                        .child(desc),
                )
            })
            .child(
                h_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .font_family(theme.mono_font_family.clone())
                            .text_color(theme.primary)
                            .child(usage_text.clone()),
                    )
                    .child(
                        Button::new(("env-copy-usage", id as u64))
                            .accessibility_label(format!("复制引用 {}", var.name))
                            .ghost()
                            .xsmall()
                            .icon(IconName::Copy)
                            .tooltip(format!("复制 {usage_text}"))
                            .on_click(move |_, window, cx| copy(usage_text.clone(), window, cx)),
                    ),
            )
    }
}

impl Render for EnvBrowser {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let keyword = self.search.read(cx).value().trim().to_lowercase();
        let all = self.state.read(cx).env_variables(self.server_id);
        let filtered: Vec<&EnvVariable> = all
            .iter()
            .filter(|v| {
                keyword.is_empty()
                    || v.name.to_lowercase().contains(&keyword)
                    || v.description
                        .as_ref()
                        .is_some_and(|d| d.to_lowercase().contains(&keyword))
            })
            .collect();
        let new_state = self.state.clone();
        let server_id = self.server_id;

        v_flex()
            .size_full()
            .gap_3()
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        div().flex_1().child(
                            Input::new(&self.search)
                                .aria_label("搜索环境变量")
                                .cleanable(true)
                                .prefix(Icon::new(IconName::Search).small().text_color(theme.muted_foreground)),
                        ),
                    )
                    .child(
                        Button::new("env-new")
                            .primary()
                            .icon(IconName::Plus)
                            .label("新增变量")
                            .on_click(move |_, window, cx| open_dialog(new_state.clone(), server_id, None, window, cx)),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("在订阅 Topic、发布的 Topic / Payload、命令模板和定时发布中用 {{变量名}} 引用，脚本中用 env.变量名 访问"),
            )
            .child(
                v_flex()
                    .id("env-cards")
                    .flex_1()
                    .min_h_0()
                    .gap_2()
                    .overflow_y_scrollbar()
                    .when(filtered.is_empty(), |this| {
                        this.child(div().h(px(240.)).child(empty_state(
                            Lucide::Variable,
                            if all.is_empty() {
                                "暂无环境变量"
                            } else {
                                "没有匹配的变量"
                            },
                            cx,
                        )))
                    })
                    .children(filtered.into_iter().map(|v| self.render_card(v, cx))),
            )
    }
}

pub fn open(state: Entity<AppState>, server_id: i64, window: &mut Window, cx: &mut App) {
    let browser = cx.new(|cx| EnvBrowser::new(state, server_id, window, cx));
    window.open_sheet_at(Placement::Right, cx, move |sheet, _, _| {
        sheet.title("环境变量").size(px(460.)).child(browser.clone())
    });
}

struct EnvForm {
    state: Entity<AppState>,
    server_id: i64,
    id: Option<i64>,
    name: Entity<InputState>,
    value: Entity<TextareaState>,
    description: Entity<InputState>,
}

impl EnvForm {
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let name = input_value(&self.name, cx);
        let value = self.value.read(cx).value().to_string();
        let description = opt_value(&self.description, cx);
        let (server_id, id) = (self.server_id, self.id);
        match self.state.update(cx, |state, cx| {
            state.save_env_variable(server_id, id, name, value, description, cx)
        }) {
            Ok(()) => {
                notify(window, cx, ToastKind::Success, "变量已保存");
                true
            }
            Err(err) => {
                notify(window, cx, ToastKind::Warning, err);
                false
            }
        }
    }
}

impl Render for EnvForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_4()
            .child(form_row(
                "变量名",
                true,
                v_flex()
                    .gap_1()
                    .child(Input::new(&self.name).aria_label("变量名"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("以字母开头，只能包含字母、数字、下划线"),
                    ),
                cx,
            ))
            .child(form_row(
                "变量值",
                true,
                Textarea::new(&self.value).aria_label("变量值"),
                cx,
            ))
            .child(form_row(
                "描述",
                false,
                Input::new(&self.description).aria_label("变量描述"),
                cx,
            ))
    }
}

/// Create (`existing` = None) or edit a variable.
pub fn open_dialog(
    state: Entity<AppState>,
    server_id: i64,
    existing: Option<EnvVariable>,
    window: &mut Window,
    cx: &mut App,
) {
    let var = existing.clone();
    let name = cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder("请输入变量名")
            .default_value(var.as_ref().map(|v| v.name.clone()).unwrap_or_default())
    });
    let value = cx.new(|cx| {
        TextareaState::new(window, cx)
            .rows(3)
            .placeholder("请输入变量值")
            .default_value(var.as_ref().map(|v| v.value.clone()).unwrap_or_default())
    });
    let description = cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder("请输入变量描述（可选）")
            .default_value(var.as_ref().and_then(|v| v.description.clone()).unwrap_or_default())
    });
    let form = cx.new(|_| EnvForm {
        state,
        server_id,
        id: existing.as_ref().and_then(|v| v.id),
        name: name.clone(),
        value,
        description,
    });
    let title = if existing.is_some() {
        "编辑变量"
    } else {
        "新增变量"
    };
    let first = std::cell::Cell::new(true);
    window.open_dialog(cx, move |dialog, window, cx| {
        if first.replace(false) {
            name.update(cx, |s, cx| s.focus(window, cx));
        }
        let save_form = form.clone();
        dialog
            .title(title)
            .w(px(520.))
            .child(form.clone())
            .footer(dialog_footer("保存"))
            .on_ok(move |_, window, cx| save_form.update(cx, |form, cx| form.save(window, cx)))
    });
}
