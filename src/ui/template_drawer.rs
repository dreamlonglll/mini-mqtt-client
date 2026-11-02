//! Template manager drawer (right-side sheet).

use super::common::{IconName, Lucide, chip, confirm, empty_state};
use super::template_dialog;
use crate::backend::models::{CommandTemplate, PayloadFormat, TemplateDraft};
use crate::backend::payload;
use crate::state::AppState;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, Icon, Placement, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::component::{Disableable as _, StyledExt as _};
use gpui_kit::{prelude::*, *};

struct TemplateBrowser {
    state: Entity<AppState>,
    server_id: i64,
    search: Entity<InputState>,
    category: Option<String>,
    _subscriptions: Vec<Subscription>,
}

impl TemplateBrowser {
    fn new(state: Entity<AppState>, server_id: i64, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("搜索名称 / Topic..."));
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
            category: None,
            _subscriptions: subscriptions,
        }
    }

    fn import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("导入模板".into()),
        });
        let (state, server_id) = (self.state.clone(), self.server_id);
        cx.spawn_in(window, async move |_, cx| {
            if let Ok(Ok(Some(paths))) = paths.await
                && let Some(path) = paths.into_iter().next()
            {
                state.update(cx, |state, cx| state.import_templates(server_id, &path, cx));
            }
        })
        .detach();
    }

    fn export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let dir = dirs::document_dir().or_else(dirs::home_dir).unwrap_or_default();
        let path = cx.prompt_for_new_path(&dir, Some("mqtt-templates.json"));
        let (state, server_id) = (self.state.clone(), self.server_id);
        cx.spawn_in(window, async move |_, cx| {
            if let Ok(Ok(Some(path))) = path.await {
                state.update(cx, |state, cx| state.export_templates(server_id, &path, cx));
            }
        })
        .detach();
    }

    fn render_card(&self, t: &CommandTemplate, cx: &App) -> impl IntoElement {
        let theme = cx.theme();
        let id = t.id.unwrap_or_default();
        let format_color = match t.format() {
            PayloadFormat::Json => theme.primary,
            PayloadFormat::Hex => theme.warning,
            PayloadFormat::Text => theme.muted_foreground,
        };
        let use_state = self.state.clone();
        let menu_state = self.state.clone();
        let template = t.clone();

        v_flex()
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
                            .child(t.name.clone()),
                    )
                    .when_some(t.category.clone(), |this, cat| this.child(chip(cat, theme.info, cx)))
                    .child(
                        Button::new(("tpl-menu", id as u64))
                            .ghost()
                            .xsmall()
                            .icon(IconName::Ellipsis)
                            .dropdown_menu(move |menu, _, _| {
                                let (edit_state, dup_state, del_state) =
                                    (menu_state.clone(), menu_state.clone(), menu_state.clone());
                                let edit_template = template.clone();
                                let name = template.name.clone();
                                menu.item(PopupMenuItem::new("编辑").icon(Lucide::SquarePen).on_click(
                                    move |_, window, cx| {
                                        template_dialog::open_edit(edit_state.clone(), &edit_template, window, cx)
                                    },
                                ))
                                .item(
                                    PopupMenuItem::new("复制")
                                        .icon(IconName::Copy)
                                        .on_click(move |_, _, cx| {
                                            dup_state.update(cx, |state, cx| state.duplicate_template(id, cx))
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
                                                "删除确认",
                                                format!("确定要删除模板「{name}」吗？"),
                                                "删除",
                                                move |_, cx| {
                                                    del_state.update(cx, |state, cx| state.delete_template(id, cx))
                                                },
                                            );
                                        }),
                                )
                            }),
                    ),
            )
            .when_some(t.description.clone().filter(|d| !d.is_empty()), |this, desc| {
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
                    .gap_1p5()
                    .text_xs()
                    .child(Icon::new(Lucide::Send).xsmall().text_color(theme.muted_foreground))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_family(theme.mono_font_family.clone())
                            .child(t.topic.clone()),
                    )
                    .child(chip(t.format().label(), format_color, cx))
                    .child(chip(format!("Q{}", t.qos), theme.primary, cx))
                    .when(t.retain, |this| this.child(chip("R", theme.warning, cx))),
            )
            .when(!t.payload.is_empty(), |this| {
                this.child(
                    div()
                        .px_2()
                        .py_1()
                        .rounded(px(4.))
                        .bg(theme.muted.opacity(0.6))
                        .text_xs()
                        .font_family(theme.mono_font_family.clone())
                        .line_clamp(3)
                        .child(payload::truncate(&t.payload, 300)),
                )
            })
            .child(
                h_flex()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(format!("使用 {} 次", t.use_count)),
                    )
                    .child(
                        Button::new(("tpl-use", id as u64))
                            .primary()
                            .xsmall()
                            .icon(Lucide::Send)
                            .label("使用")
                            .on_click(move |_, window, cx| {
                                use_state.update(cx, |state, cx| state.use_template(id, cx));
                                window.close_sheet(cx);
                            }),
                    ),
            )
    }
}

impl Render for TemplateBrowser {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let state = self.state.read(cx);
        let keyword = self.search.read(cx).value().trim().to_lowercase();
        let all = state.templates(self.server_id);
        let categories = state.storage.template_categories(self.server_id);
        let filtered: Vec<CommandTemplate> = all
            .iter()
            .filter(|t| self.category.as_ref().is_none_or(|c| t.category.as_ref() == Some(c)))
            .filter(|t| {
                keyword.is_empty()
                    || t.name.to_lowercase().contains(&keyword)
                    || t.topic.to_lowercase().contains(&keyword)
                    || t.description
                        .as_ref()
                        .is_some_and(|d| d.to_lowercase().contains(&keyword))
            })
            .cloned()
            .collect();
        let entity = cx.entity();
        let (e_import, e_export) = (entity.clone(), entity.clone());
        let new_state = self.state.clone();
        let server_id = self.server_id;
        let has_templates = !all.is_empty();

        let category_button = |label: String, value: Option<String>, ix: usize, selected: bool| {
            let entity = entity.clone();
            let button = Button::new(("tpl-cat", ix)).xsmall().label(label);
            let button = if selected { button.primary() } else { button.outline() };
            button.on_click(move |_, _, cx| {
                entity.update(cx, |this, cx| {
                    this.category = value.clone();
                    cx.notify();
                })
            })
        };

        v_flex()
            .size_full()
            .gap_3()
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        div().flex_1().child(
                            Input::new(&self.search)
                                .cleanable(true)
                                .prefix(Icon::new(IconName::Search).small().text_color(theme.muted_foreground)),
                        ),
                    )
                    .child(
                        Button::new("tpl-new")
                            .primary()
                            .icon(IconName::Plus)
                            .label("新建")
                            .on_click(move |_, window, cx| {
                                template_dialog::open(
                                    new_state.clone(),
                                    server_id,
                                    None,
                                    TemplateDraft::default(),
                                    window,
                                    cx,
                                )
                            }),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("tpl-import")
                            .outline()
                            .small()
                            .icon(Lucide::Upload)
                            .label("导入")
                            .on_click(move |_, window, cx| e_import.update(cx, |this, cx| this.import(window, cx))),
                    )
                    .child(
                        Button::new("tpl-export")
                            .outline()
                            .small()
                            .icon(Lucide::Download)
                            .label("导出")
                            .disabled(!has_templates)
                            .on_click(move |_, window, cx| e_export.update(cx, |this, cx| this.export(window, cx))),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(format!("共 {} 个模板", all.len())),
                    ),
            )
            .when(!categories.is_empty(), |this| {
                this.child(
                    h_flex()
                        .flex_wrap()
                        .gap_1p5()
                        .child(category_button("全部".into(), None, 0, self.category.is_none()))
                        .children(categories.iter().enumerate().map(|(ix, c)| {
                            category_button(c.clone(), Some(c.clone()), ix + 1, self.category.as_ref() == Some(c))
                        })),
                )
            })
            .child(
                v_flex()
                    .id("template-cards")
                    .flex_1()
                    .min_h_0()
                    .gap_2()
                    .overflow_y_scrollbar()
                    .when(filtered.is_empty(), |this| {
                        this.child(div().h(px(240.)).child(empty_state(
                            IconName::Inbox,
                            if has_templates {
                                "没有匹配的模板"
                            } else {
                                "暂无模板，点击「新建」创建常用命令"
                            },
                            cx,
                        )))
                    })
                    .children(filtered.iter().map(|t| self.render_card(t, cx))),
            )
    }
}

pub fn open(state: Entity<AppState>, server_id: i64, window: &mut Window, cx: &mut App) {
    let browser = cx.new(|cx| TemplateBrowser::new(state, server_id, window, cx));
    window.open_sheet_at(Placement::Right, cx, move |sheet, _, _| {
        sheet.title("命令模板").size(px(460.)).child(browser.clone())
    });
}
