//! Script manager: list of scripts, code editor, test run and API help.

use super::common::{IconName, Lucide, card, chip, confirm, empty_state, form_row, input_value, notify, opt_value};
use crate::backend::models::{Script, ScriptType};
use crate::backend::script::{self, ScriptOutput};
use crate::state::{AppState, ToastKind};
use gpui_kit::component::StyledExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::DialogFooter;
use gpui_kit::component::input::{Editor, EditorState, Input, InputState};
use gpui_kit::component::radio::{Radio, RadioGroup};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::{prelude::*, *};

fn default_code(kind: ScriptType) -> &'static str {
    match kind {
        ScriptType::BeforePublish => {
            "// 定义 process 函数处理 payload，点击\"查看可用函数\"查看更多\n\nasync function process(payload) {\n  // 在这里处理 payload\n  return payload;\n}\n"
        }
        ScriptType::AfterReceive => {
            "// 定义 process 函数处理 payload，点击\"查看可用函数\"查看更多\n\nasync function process(payload, topic) {\n  // 在这里处理接收到的消息\n  return payload;\n}\n"
        }
    }
}

struct ScriptManager {
    state: Entity<AppState>,
    server_id: i64,
    /// `None` while creating a new script.
    selected: Option<i64>,
    kind: ScriptType,
    name: Entity<InputState>,
    description: Entity<InputState>,
    code: Entity<EditorState>,
    sample: Entity<InputState>,
    output: Option<ScriptOutput>,
    testing: bool,
    _subscription: Subscription,
}

impl ScriptManager {
    fn new(state: Entity<AppState>, server_id: i64, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("脚本名称"));
        let description = cx.new(|cx| InputState::new(window, cx).placeholder("脚本描述（可选）"));
        let code = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("javascript")
                .line_number(true)
                .tab_size(gpui_kit::component::input::TabSize {
                    tab_size: 2,
                    hard_tabs: false,
                })
                .default_value(default_code(ScriptType::BeforePublish))
        });
        let sample = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("测试用 payload")
                .default_value(r#"{"hello": "mqtt"}"#)
        });
        let subscription = cx.observe(&state, |_, _, cx| cx.notify());
        let mut this = Self {
            state,
            server_id,
            selected: None,
            kind: ScriptType::BeforePublish,
            name,
            description,
            code,
            sample,
            output: None,
            testing: false,
            _subscription: subscription,
        };
        if let Some(first) = this.state.read(cx).scripts(server_id).into_iter().next() {
            this.select(&first, window, cx);
        }
        this
    }

    fn select(&mut self, script: &Script, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = script.id;
        self.kind = script.kind();
        self.output = None;
        let (name, desc, code) = (
            script.name.clone(),
            script.description.clone().unwrap_or_default(),
            script.code.clone(),
        );
        self.name.update(cx, |s, cx| s.set_value(name, window, cx));
        self.description.update(cx, |s, cx| s.set_value(desc, window, cx));
        self.code.update(cx, |s, cx| s.set_value(code, window, cx));
        cx.notify();
    }

    fn start_new(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = None;
        self.kind = ScriptType::BeforePublish;
        self.output = None;
        self.name.update(cx, |s, cx| s.set_value("", window, cx));
        self.description.update(cx, |s, cx| s.set_value("", window, cx));
        self.code.update(cx, |s, cx| {
            s.set_value(default_code(ScriptType::BeforePublish), window, cx)
        });
        self.name.update(cx, |s, cx| s.focus(window, cx));
        cx.notify();
    }

    fn set_kind(&mut self, kind: ScriptType, window: &mut Window, cx: &mut Context<Self>) {
        // Swap the template code only if the user has not edited it yet.
        let current = self.code.read(cx).value().to_string();
        if current.trim() == default_code(self.kind).trim() || current.trim().is_empty() {
            self.code
                .update(cx, |s, cx| s.set_value(default_code(kind), window, cx));
        }
        self.kind = kind;
        cx.notify();
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = input_value(&self.name, cx).trim().to_string();
        let description = opt_value(&self.description, cx);
        let code = self.code.read(cx).value().to_string();
        let (server_id, id, kind) = (self.server_id, self.selected, self.kind);
        let result = self.state.update(cx, |state, cx| {
            state.save_script(server_id, id, name, kind, code, description, cx)
        });
        match result {
            Ok(id) => {
                self.selected = Some(id);
                notify(window, cx, ToastKind::Success, "脚本已保存");
            }
            Err(err) => notify(window, cx, ToastKind::Warning, err),
        }
        cx.notify();
    }

    fn delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.selected else {
            return;
        };
        let name = input_value(&self.name, cx);
        let entity = cx.entity();
        let state = self.state.clone();
        confirm(
            window,
            cx,
            "删除脚本",
            format!("确定要删除脚本「{name}」吗？"),
            "删除",
            move |window, cx| {
                state.update(cx, |state, cx| state.delete_script(id, cx));
                entity.update(cx, |this, cx| this.start_new(window, cx));
            },
        );
    }

    fn test_run(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let code = self.code.read(cx).value().to_string();
        let payload = input_value(&self.sample, cx);
        self.testing = true;
        cx.notify();
        let task = cx
            .background_executor()
            .spawn(async move { script::run_script(&code, &payload, "test/topic") });
        cx.spawn_in(window, async move |this, cx| {
            let output = task.await;
            let _ = this.update(cx, |this, cx| {
                this.testing = false;
                this.output = Some(output);
                cx.notify();
            });
        })
        .detach();
    }

    fn render_list(&self, scripts: &[Script], cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let entity = cx.entity();
        let new_entity = entity.clone();
        card(cx)
            .w(px(260.))
            .flex_none()
            .h_full()
            .child(
                h_flex()
                    .flex_none()
                    .h(px(48.))
                    .px_3()
                    .justify_between()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(div().font_semibold().child("脚本列表"))
                    .child(
                        Button::new("script-new")
                            .primary()
                            .small()
                            .icon(IconName::Plus)
                            .label("新增")
                            .on_click(move |_, window, cx| {
                                new_entity.update(cx, |this, cx| this.start_new(window, cx))
                            }),
                    ),
            )
            .child(
                v_flex()
                    .id("script-list")
                    .flex_1()
                    .min_h_0()
                    .p_1()
                    .gap_0p5()
                    .overflow_y_scrollbar()
                    .when(scripts.is_empty(), |this| {
                        this.child(div().h(px(200.)).child(empty_state(Lucide::FileCode, "暂无脚本", cx)))
                    })
                    .children(scripts.iter().map(|s| {
                        let id = s.id.unwrap_or_default();
                        let selected = self.selected == Some(id);
                        let select_entity = entity.clone();
                        let script = s.clone();
                        let toggle_state = self.state.clone();
                        let kind_color = match s.kind() {
                            ScriptType::BeforePublish => theme.primary,
                            ScriptType::AfterReceive => theme.success,
                        };
                        h_flex()
                            .id(("script", id as u64))
                            .px_3()
                            .py_2()
                            .gap_2()
                            .rounded(theme.radius)
                            .cursor_pointer()
                            .when(selected, |this| this.bg(theme.accent))
                            .when(!selected, |this| this.hover(|st| st.bg(theme.accent.opacity(0.5))))
                            .on_click(move |_, window, cx| {
                                select_entity.update(cx, |this, cx| this.select(&script, window, cx))
                            })
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap_1()
                                    .child(div().text_sm().truncate().child(s.name.clone()))
                                    .child(h_flex().child(chip(s.kind().label(), kind_color, cx))),
                            )
                            .child(
                                Switch::new(("script-enabled", id as u64))
                                    .small()
                                    .checked(s.enabled)
                                    .on_click(move |checked, _, cx| {
                                        let checked = *checked;
                                        toggle_state.update(cx, |state, cx| state.toggle_script(id, checked, cx))
                                    }),
                            )
                    })),
            )
    }

    fn render_output(&self, cx: &App) -> Option<impl IntoElement> {
        let theme = cx.theme();
        let output = self.output.as_ref()?;
        let (color, title, body) = match &output.result {
            Ok(result) => (theme.success, "运行成功，输出：", result.clone()),
            Err(err) => (theme.danger, "运行失败：", err.clone()),
        };
        Some(
            v_flex()
                .id("script-output")
                .max_h(px(96.))
                .overflow_y_scrollbar()
                .p_2()
                .gap_1()
                .rounded(theme.radius)
                .border_1()
                .border_color(color.opacity(0.4))
                .bg(color.opacity(0.06))
                .text_xs()
                .font_family(theme.mono_font_family.clone())
                .child(div().text_color(color).font_semibold().child(title))
                .child(div().child(body))
                .when(!output.console.is_empty(), |this| {
                    this.child(div().mt_1().text_color(theme.muted_foreground).child("console:"))
                        .children(output.console.iter().map(|line| div().child(line.clone())))
                }),
        )
    }
}

impl Render for ScriptManager {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let scripts = self.state.read(cx).scripts(self.server_id);
        let entity = cx.entity();
        let (e_kind, e_delete, e_save, e_test) = (entity.clone(), entity.clone(), entity.clone(), entity.clone());
        let editing = self.selected.is_some();
        let hint = match self.kind {
            ScriptType::BeforePublish => "定义 process(payload) 函数，返回处理后的 payload（发送前执行）",
            ScriptType::AfterReceive => "定义 process(payload, topic) 函数，返回处理后的 payload（接收后执行）",
        };

        h_flex()
            .h(px(540.))
            .gap_4()
            .child(self.render_list(&scripts, cx))
            .child(
                card(cx)
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(
                        h_flex()
                            .flex_none()
                            .h(px(48.))
                            .px_4()
                            .justify_between()
                            .border_b_1()
                            .border_color(theme.border)
                            .child(
                                div()
                                    .font_semibold()
                                    .child(if editing { "编辑脚本" } else { "新建脚本" }),
                            )
                            .when(editing, |this| {
                                this.child(
                                    Button::new("script-delete")
                                        .ghost()
                                        .small()
                                        .icon(Lucide::Trash)
                                        .label("删除")
                                        .on_click(move |_, window, cx| {
                                            e_delete.update(cx, |this, cx| this.delete(window, cx))
                                        }),
                                )
                            }),
                    )
                    .child(
                        v_flex()
                            .id("script-editor-body")
                            .flex_1()
                            .min_h_0()
                            .p_4()
                            .gap_3()
                            .overflow_y_scrollbar()
                            .child(form_row(
                                "名称",
                                true,
                                h_flex()
                                    .gap_4()
                                    .child(div().flex_1().child(Input::new(&self.name)))
                                    .child(
                                        RadioGroup::horizontal("script-kind")
                                            .disabled(editing)
                                            .selected_index(Some(match self.kind {
                                                ScriptType::BeforePublish => 0,
                                                ScriptType::AfterReceive => 1,
                                            }))
                                            .on_click(move |ix, window, cx| {
                                                let kind = if *ix == 1 {
                                                    ScriptType::AfterReceive
                                                } else {
                                                    ScriptType::BeforePublish
                                                };
                                                e_kind.update(cx, |this, cx| this.set_kind(kind, window, cx))
                                            })
                                            .child(Radio::new("kind-before").label("发送前处理"))
                                            .child(Radio::new("kind-after").label("接收后处理")),
                                    ),
                                cx,
                            ))
                            .child(form_row("描述", false, Input::new(&self.description), cx))
                            .child(
                                div()
                                    .px_3()
                                    .py_2()
                                    .rounded(theme.radius)
                                    .bg(theme.muted)
                                    .text_xs()
                                    .font_family(theme.mono_font_family.clone())
                                    .text_color(theme.muted_foreground)
                                    .child(hint),
                            )
                            .child(div().flex_1().min_h(px(130.)).child(Editor::new(&self.code).h_full()))
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(div().flex_1().child(Input::new(&self.sample).small()))
                                    .child(
                                        Button::new("script-test")
                                            .outline()
                                            .small()
                                            .icon(IconName::Play)
                                            .label("测试运行")
                                            .loading(self.testing)
                                            .on_click(move |_, window, cx| {
                                                e_test.update(cx, |this, cx| this.test_run(window, cx))
                                            }),
                                    ),
                            )
                            .children(self.render_output(cx)),
                    )
                    .child(
                        h_flex()
                            .flex_none()
                            .px_4()
                            .py_3()
                            .gap_2()
                            .justify_end()
                            .border_t_1()
                            .border_color(theme.border)
                            .child(
                                Button::new("script-help")
                                    .outline()
                                    .icon(IconName::BookOpen)
                                    .label("查看可用函数")
                                    .on_click(|_, window, cx| open_help(window, cx)),
                            )
                            .child(
                                Button::new("script-save")
                                    .primary()
                                    .icon(Lucide::Save)
                                    .label("保存")
                                    .on_click(move |_, window, cx| e_save.update(cx, |this, cx| this.save(window, cx))),
                            ),
                    ),
            )
    }
}

pub fn open(state: Entity<AppState>, server_id: i64, window: &mut Window, cx: &mut App) {
    let manager = cx.new(|cx| ScriptManager::new(state, server_id, window, cx));
    window.open_dialog(cx, move |dialog, _, _| {
        dialog
            .title("脚本管理")
            .w(px(1040.))
            .child(manager.clone())
            .footer(DialogFooter::new().child(super::common::close_button("关闭", false)))
    });
}

const HELP: &[(&str, &[(&str, &str)])] = &[
    (
        "入口",
        &[
            (
                "process(payload, topic)",
                "返回值替换 payload；可为 async。返回对象时自动 JSON.stringify，返回 undefined 保持原样",
            ),
            ("payload / topic", "全局变量：原始消息内容（字符串）与主题"),
            ("console.log / warn / error", "输出日志（测试运行时可见）"),
            ("atob / btoa", "Base64 编解码（Latin1）"),
            ("JSON, Math, Date, ...", "标准 JavaScript 内置对象"),
        ],
    ),
    (
        "编码转换",
        &[
            ("crypto.stringToBytes(str)", "字符串 → Uint8Array (UTF-8)"),
            ("crypto.bytesToString(bytes)", "Uint8Array → 字符串"),
            ("crypto.bytesToBase64(bytes) / base64ToBytes(str)", "Base64 编解码"),
            ("crypto.bytesToHex(bytes) / hexToBytes(hex)", "Hex 编解码"),
        ],
    ),
    (
        "哈希",
        &[
            ("await crypto.sha256(data)", "SHA-256，返回小写 hex"),
            ("await crypto.sha1(data)", "SHA-1，返回小写 hex"),
            ("crypto.md5(data)", "MD5，返回小写 hex"),
            ("await crypto.hmacSha256(key, data)", "HMAC-SHA256，返回小写 hex"),
            ("crypto.crc32(data)", "CRC32，返回 8 位 hex"),
        ],
    ),
    (
        "AES（Base64 格式，结果包含 IV）",
        &[
            (
                "await crypto.aesGcmEncrypt(text, keyBase64, ivBase64?)",
                "AES-GCM 加密，IV 12 字节，缺省随机",
            ),
            ("await crypto.aesGcmDecrypt(cipherBase64, keyBase64)", "AES-GCM 解密"),
            (
                "await crypto.aesCbcEncrypt(text, keyBase64, ivBase64?)",
                "AES-CBC/PKCS7 加密，IV 16 字节",
            ),
            ("await crypto.aesCbcDecrypt(cipherBase64, keyBase64)", "AES-CBC 解密"),
        ],
    ),
    (
        "AES（Hex 格式，结果包含 IV）",
        &[
            ("await crypto.aesGcmEncryptHex(text, keyHex, ivHex?)", "AES-GCM 加密"),
            ("await crypto.aesGcmDecryptHex(cipherHex, keyHex)", "AES-GCM 解密"),
            ("await crypto.aesCbcEncryptHex(text, keyHex, ivHex?)", "AES-CBC 加密"),
            ("await crypto.aesCbcDecryptHex(cipherHex, keyHex)", "AES-CBC 解密"),
        ],
    ),
    (
        "其他",
        &[
            ("crypto.randomBytes(length)", "随机字节 Uint8Array"),
            ("crypto.generateKey(bits = 128)", "随机密钥（Base64）"),
            ("crypto.generateIv(length = 12)", "随机 IV（Base64）"),
            ("crypto.xor(data, key)", "XOR 加解密（对称）"),
        ],
    ),
];

fn open_help(window: &mut Window, cx: &mut App) {
    window.open_dialog(cx, |dialog, _, cx| {
        let theme = cx.theme().clone();
        dialog
            .title("可用函数")
            .w(px(760.))
            .child(
                v_flex()
                    .id("script-help-body")
                    .max_h(px(520.))
                    .overflow_y_scrollbar()
                    .gap_4()
                    .children(HELP.iter().map(|(section, items)| {
                        v_flex()
                            .gap_1()
                            .child(div().text_sm().font_semibold().child(*section))
                            .children(items.iter().map(|(sig, desc)| {
                                h_flex()
                                    .gap_3()
                                    .items_start()
                                    .text_xs()
                                    .child(
                                        div()
                                            .w(px(360.))
                                            .flex_none()
                                            .font_family(theme.mono_font_family.clone())
                                            .text_color(theme.primary)
                                            .child(*sig),
                                    )
                                    .child(div().flex_1().text_color(theme.muted_foreground).child(*desc))
                            }))
                    }))
                    .child(
                        v_flex()
                            .gap_1()
                            .child(div().text_sm().font_semibold().child("示例"))
                            .child(
                                div()
                                    .p_2()
                                    .rounded(theme.radius)
                                    .bg(theme.muted)
                                    .text_xs()
                                    .font_family(theme.mono_font_family.clone())
                                    .child(
                                        "async function process(payload) {\n  const key = \"8568230E4BBAF3A979B6F510A38717D5\";\n  return await crypto.aesGcmEncryptHex(payload, key);\n}",
                                    ),
                            ),
                    ),
            )
            .footer(DialogFooter::new().child(super::common::close_button("知道了", true)))
    });
}
