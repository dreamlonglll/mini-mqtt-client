//! Create / edit command template dialog.

use super::common::{
    IconName, Lucide, dialog_footer, form_row, input_value, notify, opt_value, section_label, segmented,
};
use crate::backend::models::{CommandTemplate, PayloadFormat, TemplateDraft};
use crate::backend::payload;
use crate::state::{AppState, ToastKind};
use gpui_kit::component::StyledExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Editor, EditorState, Input, InputState};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::{prelude::*, *};

struct TemplateForm {
    state: Entity<AppState>,
    server_id: i64,
    id: Option<i64>,
    name: Entity<InputState>,
    category: Entity<InputState>,
    description: Entity<InputState>,
    topic: Entity<InputState>,
    payload: Entity<EditorState>,
    categories: Vec<String>,
    qos: u8,
    retain: bool,
    format: PayloadFormat,
}

impl TemplateForm {
    fn new(
        state: Entity<AppState>,
        server_id: i64,
        id: Option<i64>,
        draft: TemplateDraft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let text = |window: &mut Window, cx: &mut Context<Self>, value: &str, placeholder: &str| {
            let (value, placeholder) = (value.to_string(), placeholder.to_string());
            cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder(placeholder)
                    .default_value(value)
            })
        };
        let categories = state.read(cx).storage.template_categories(server_id);
        let name = text(window, cx, &draft.name, "给模板起个容易识别的名称");
        let category = text(
            window,
            cx,
            draft.category.as_deref().unwrap_or(""),
            "输入新分类或点击下方已有分类",
        );
        let description = text(
            window,
            cx,
            draft.description.as_deref().unwrap_or(""),
            "可选：描述模板的用途",
        );
        let topic = text(window, cx, &draft.topic, "例如: device/001/command");
        let payload_text = draft.payload.clone();
        let payload = cx.new(|cx| {
            EditorState::new(window, cx)
                .line_number(true)
                .soft_wrap(true)
                .default_value(payload_text)
        });
        Self {
            state,
            server_id,
            id,
            name,
            category,
            description,
            topic,
            payload,
            categories,
            qos: draft.qos.clamp(0, 2) as u8,
            retain: draft.retain,
            format: draft.payload_type,
        }
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let draft = TemplateDraft {
            name: input_value(&self.name, cx).trim().to_string(),
            topic: input_value(&self.topic, cx).trim().to_string(),
            payload: self.payload.read(cx).value().to_string(),
            payload_type: self.format,
            qos: self.qos as i32,
            retain: self.retain,
            description: opt_value(&self.description, cx),
            category: opt_value(&self.category, cx),
        };
        let (server_id, id) = (self.server_id, self.id);
        match self
            .state
            .update(cx, |state, cx| state.save_template(server_id, id, draft, cx))
        {
            Ok(()) => {
                notify(
                    window,
                    cx,
                    ToastKind::Success,
                    if id.is_some() {
                        "模板已更新"
                    } else {
                        "模板已创建"
                    },
                );
                true
            }
            Err(err) => {
                notify(window, cx, ToastKind::Warning, err);
                false
            }
        }
    }

    fn reformat(&mut self, pretty: bool, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.payload.read(cx).value().to_string();
        let formatted = if pretty {
            payload::pretty_json(&text)
        } else {
            payload::compact_json(&text)
        };
        match formatted {
            Some(f) => self.payload.update(cx, |s, cx| s.set_value(f, window, cx)),
            None => notify(window, cx, ToastKind::Warning, "JSON 格式不正确"),
        }
    }
}

impl Render for TemplateForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let entity = cx.entity();
        let (e_qos, e_retain, e_format, e_pretty, e_compact) = (
            entity.clone(),
            entity.clone(),
            entity.clone(),
            entity.clone(),
            entity.clone(),
        );
        let format_ix = PayloadFormat::ALL.iter().position(|f| *f == self.format).unwrap_or(0);

        v_flex()
            .gap_3()
            .child(section_label("基本信息", cx))
            .child(form_row("模板名称", true, Input::new(&self.name), cx))
            .child(form_row(
                "分类",
                false,
                v_flex()
                    .gap_1p5()
                    .child(Input::new(&self.category))
                    .when(!self.categories.is_empty(), |this| {
                        this.child(
                            h_flex()
                                .flex_wrap()
                                .gap_1()
                                .children(self.categories.iter().enumerate().map(|(ix, cat)| {
                                    let category = self.category.clone();
                                    let value = cat.clone();
                                    Button::new(("cat", ix)).ghost().xsmall().label(cat.clone()).on_click(
                                        move |_, window, cx| {
                                            category.update(cx, |s, cx| s.set_value(value.clone(), window, cx))
                                        },
                                    )
                                })),
                        )
                    }),
                cx,
            ))
            .child(form_row("描述", false, Input::new(&self.description), cx))
            .child(section_label("MQTT 配置", cx))
            .child(form_row(
                "主题",
                true,
                Input::new(&self.topic).prefix(
                    gpui_kit::component::Icon::new(Lucide::Send)
                        .small()
                        .text_color(theme.muted_foreground),
                ),
                cx,
            ))
            .child(form_row(
                "QoS",
                false,
                h_flex()
                    .gap_6()
                    .child(segmented(
                        "tpl-qos",
                        &["QoS 0", "QoS 1", "QoS 2"],
                        self.qos as usize,
                        move |ix, _, cx| {
                            e_qos.update(cx, |this, cx| {
                                this.qos = ix as u8;
                                cx.notify();
                            })
                        },
                    ))
                    .child(
                        Switch::new("tpl-retain")
                            .label("Retain 保留消息")
                            .checked(self.retain)
                            .on_click(move |checked, _, cx| {
                                let checked = *checked;
                                e_retain.update(cx, |this, cx| {
                                    this.retain = checked;
                                    cx.notify();
                                })
                            }),
                    ),
                cx,
            ))
            .child(
                h_flex()
                    .justify_between()
                    .border_b_1()
                    .border_color(theme.border)
                    .pb_1()
                    .child(div().text_sm().font_semibold().child("消息内容"))
                    .child(
                        h_flex()
                            .gap_1()
                            .when(self.format == PayloadFormat::Json, |this| {
                                this.child(
                                    Button::new("tpl-pretty")
                                        .ghost()
                                        .xsmall()
                                        .icon(Lucide::Braces)
                                        .label("格式化")
                                        .on_click(move |_, window, cx| {
                                            e_pretty.update(cx, |this, cx| this.reformat(true, window, cx))
                                        }),
                                )
                                .child(
                                    Button::new("tpl-compact")
                                        .ghost()
                                        .xsmall()
                                        .icon(IconName::Minus)
                                        .label("压缩")
                                        .on_click(move |_, window, cx| {
                                            e_compact.update(cx, |this, cx| this.reformat(false, window, cx))
                                        }),
                                )
                            })
                            .child(segmented(
                                "tpl-format",
                                &["JSON", "TEXT", "HEX"],
                                format_ix,
                                move |ix, _, cx| {
                                    e_format.update(cx, |this, cx| {
                                        this.format = PayloadFormat::ALL[ix];
                                        cx.notify();
                                    })
                                },
                            )),
                    ),
            )
            .child(div().h(px(200.)).child(Editor::new(&self.payload).h_full()))
    }
}

/// Open the dialog. `id` = Some(..) edits an existing template.
pub fn open(
    state: Entity<AppState>,
    server_id: i64,
    id: Option<i64>,
    draft: TemplateDraft,
    window: &mut Window,
    cx: &mut App,
) {
    let title = if id.is_some() { "编辑模板" } else { "新建模板" };
    let ok = if id.is_some() { "保存" } else { "创建模板" };
    let form = cx.new(|cx| TemplateForm::new(state, server_id, id, draft, window, cx));
    window.open_dialog(cx, move |dialog, _, _| {
        let save_form = form.clone();
        dialog
            .title(title)
            .w(px(720.))
            .child(form.clone())
            .footer(dialog_footer(ok))
            .on_ok(move |_, window, cx| save_form.update(cx, |form, cx| form.save(window, cx)))
    });
}

pub fn open_edit(state: Entity<AppState>, template: &CommandTemplate, window: &mut Window, cx: &mut App) {
    let draft = TemplateDraft {
        name: template.name.clone(),
        topic: template.topic.clone(),
        payload: template.payload.clone(),
        payload_type: template.format(),
        qos: template.qos,
        retain: template.retain,
        description: template.description.clone(),
        category: template.category.clone(),
    };
    open(state, template.server_id, template.id, draft, window, cx);
}
