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
    notify(window, cx, ToastKind::Success, tr!("common.copied"));
}

struct EnvBrowser {
    state: Entity<AppState>,
    server_id: i64,
    search: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

impl EnvBrowser {
    fn new(state: Entity<AppState>, server_id: i64, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder(tr!("env.search_placeholder")));
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
            .aria_label(format!("{} {} {}", tr!("env.item"), var.name, var.value))
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
                            .accessibility_label(tr!("env.menu", name = var.name))
                            .ghost()
                            .xsmall()
                            .icon(IconName::Ellipsis)
                            .dropdown_menu(move |menu, _, _| {
                                let (edit_state, del_state) = (menu_state.clone(), menu_state.clone());
                                let (edit_var, value, name) =
                                    (menu_var.clone(), menu_var.value.clone(), menu_var.name.clone());
                                menu.item(PopupMenuItem::new(tr!("common.edit")).icon(Lucide::SquarePen).on_click(
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
                                    PopupMenuItem::new(tr!("env.copy_value"))
                                        .icon(IconName::Copy)
                                        .on_click(move |_, window, cx| copy(value.clone(), window, cx)),
                                )
                                .separator()
                                .item(
                                    PopupMenuItem::new(tr!("common.delete")).icon(Lucide::Trash).on_click(
                                        move |_, window, cx| {
                                            let del_state = del_state.clone();
                                            confirm(
                                                window,
                                                cx,
                                                tr!("env.delete_title"),
                                                tr!("env.delete_confirm", name = name),
                                                tr!("common.delete"),
                                                move |window, cx| match del_state
                                                    .update(cx, |state, cx| state.delete_env_variable(id, cx))
                                                {
                                                    Ok(()) => {
                                                        notify(window, cx, ToastKind::Success, tr!("env.deleted"))
                                                    }
                                                    Err(err) => notify(window, cx, ToastKind::Error, err),
                                                },
                                            );
                                        },
                                    ),
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
                            .accessibility_label(tr!("env.copy_usage_of", name = var.name))
                            .ghost()
                            .xsmall()
                            .icon(IconName::Copy)
                            .tooltip(tr!("env.copy_usage", usage = usage_text))
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
                                .aria_label(tr!("env.search"))
                                .cleanable(true)
                                .prefix(Icon::new(IconName::Search).small().text_color(theme.muted_foreground)),
                        ),
                    )
                    .child(
                        Button::new("env-new")
                            .primary()
                            .icon(IconName::Plus)
                            .label(tr!("env.add"))
                            .on_click(move |_, window, cx| open_dialog(new_state.clone(), server_id, None, window, cx)),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(tr!("env.hint")),
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
                                tr!("env.empty")
                            } else {
                                tr!("env.no_match")
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
        sheet.title(tr!("env.title")).size(px(460.)).child(browser.clone())
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
                notify(window, cx, ToastKind::Success, tr!("env.saved"));
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
                tr!("env.name"),
                true,
                v_flex()
                    .gap_1()
                    .child(Input::new(&self.name).aria_label(tr!("env.name")))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(tr!("env.name_hint")),
                    ),
                cx,
            ))
            .child(form_row(
                tr!("env.value"),
                true,
                Textarea::new(&self.value).aria_label(tr!("env.value")),
                cx,
            ))
            .child(form_row(
                tr!("common.description"),
                false,
                Input::new(&self.description).aria_label(tr!("env.description")),
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
            .placeholder(tr!("env.name_placeholder"))
            .default_value(var.as_ref().map(|v| v.name.clone()).unwrap_or_default())
    });
    let value = cx.new(|cx| {
        TextareaState::new(window, cx)
            .rows(3)
            .placeholder(tr!("env.value_placeholder"))
            .default_value(var.as_ref().map(|v| v.value.clone()).unwrap_or_default())
    });
    let description = cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder(tr!("env.description_placeholder"))
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
        tr!("env.edit_title")
    } else {
        tr!("env.add")
    };
    let first = std::cell::Cell::new(true);
    window.open_dialog(cx, move |dialog, window, cx| {
        if first.replace(false) {
            name.update(cx, |s, cx| s.focus(window, cx));
        }
        let save_form = form.clone();
        dialog
            .title(title.clone())
            .w(px(520.))
            .child(form.clone())
            .footer(dialog_footer(tr!("common.save")))
            .on_ok(move |_, window, cx| save_form.update(cx, |form, cx| form.save(window, cx)))
    });
}
