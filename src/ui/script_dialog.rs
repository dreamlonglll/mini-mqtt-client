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

fn default_code(kind: ScriptType) -> String {
    match kind {
        ScriptType::BeforePublish => format!(
            "// {}\n\nasync function process(payload, topic) {{\n  // {}\n  return payload;\n}}\n",
            tr!("script.default_comment"),
            tr!("script.default_before")
        ),
        ScriptType::AfterReceive => format!(
            "// {}\n\nasync function process(payload, topic) {{\n  // {}\n  return payload;\n}}\n",
            tr!("script.default_comment"),
            tr!("script.default_after")
        ),
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
        let name = cx.new(|cx| InputState::new(window, cx).placeholder(tr!("script.name")));
        let description = cx.new(|cx| InputState::new(window, cx).placeholder(tr!("script.description_placeholder")));
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
                .placeholder(tr!("script.sample_placeholder"))
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
                notify(window, cx, ToastKind::Success, tr!("script.saved"));
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
            tr!("script.delete_title"),
            tr!("script.delete_confirm", name = name),
            tr!("common.delete"),
            move |window, cx| {
                state.update(cx, |state, cx| state.delete_script(id, cx));
                entity.update(cx, |this, cx| this.start_new(window, cx));
            },
        );
    }

    fn test_run(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let code = self.code.read(cx).value().to_string();
        let payload = input_value(&self.sample, cx);
        let env = self.state.read(cx).env_pairs(self.server_id);
        self.testing = true;
        cx.notify();
        let task = cx
            .background_executor()
            .spawn(async move { script::run_script(&code, &payload, "test/topic", &env) });
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

    /// Replace the editor's code with a `.js` / `.txt` file, after a warning
    /// about running code from elsewhere.
    fn import_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let entity = cx.entity();
        confirm(
            window,
            cx,
            tr!("script.import_title"),
            tr!("script.import_warning"),
            tr!("script.import_confirm"),
            move |window, cx| {
                let paths = cx.prompt_for_paths(PathPromptOptions {
                    files: true,
                    directories: false,
                    multiple: false,
                    prompt: Some(tr!("script.import_prompt")),
                });
                let entity = entity.clone();
                window
                    .spawn(cx, async move |cx| {
                        let Ok(Ok(Some(paths))) = paths.await else {
                            return;
                        };
                        let Some(path) = paths.into_iter().next() else {
                            return;
                        };
                        let result = read_script_file(&path);
                        let _ = entity.update_in(cx, |this, window, cx| match result {
                            Ok(code) => {
                                this.code.update(cx, |s, cx| s.set_value(code, window, cx));
                                this.output = None;
                                notify(
                                    window,
                                    cx,
                                    ToastKind::Success,
                                    tr!("script.imported", file = path.display()),
                                );
                                cx.notify();
                            }
                            Err(err) => notify(window, cx, ToastKind::Error, err),
                        });
                    })
                    .detach();
            },
        );
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
                    .child(div().font_semibold().child(tr!("script.list")))
                    .child(
                        Button::new("script-new")
                            .primary()
                            .small()
                            .icon(IconName::Plus)
                            .label(tr!("script.new"))
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
                        this.child(
                            div()
                                .h(px(200.))
                                .child(empty_state(Lucide::FileCode, tr!("script.empty"), cx)),
                        )
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
                            .role(Role::ListItem)
                            .aria_label(format!(
                                "{} {} {} {}",
                                tr!("script.item"),
                                s.name,
                                s.kind().label(),
                                if s.enabled {
                                    tr!("common.enabled")
                                } else {
                                    tr!("common.disabled")
                                }
                            ))
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
                                    .accessibility_label(tr!("script.enable", name = s.name))
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
            Ok(result) => (theme.success, tr!("script.run_ok"), result.clone()),
            Err(err) => (theme.danger, tr!("script.run_failed"), err.clone()),
        };
        Some(
            v_flex()
                .id("script-output")
                .role(Role::Status)
                .aria_label(format!("{} {title}{body}", tr!("script.test_result")))
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
        let (e_kind, e_delete, e_save, e_test, e_import) = (
            entity.clone(),
            entity.clone(),
            entity.clone(),
            entity.clone(),
            entity.clone(),
        );
        let editing = self.selected.is_some();
        let hint = match self.kind {
            ScriptType::BeforePublish => tr!("script.hint_before"),
            ScriptType::AfterReceive => tr!("script.hint_after"),
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
                            .child(div().font_semibold().child(if editing {
                                tr!("script.edit_title")
                            } else {
                                tr!("script.new_title")
                            }))
                            .when(editing, |this| {
                                this.child(
                                    Button::new("script-delete")
                                        .ghost()
                                        .small()
                                        .icon(Lucide::Trash)
                                        .label(tr!("common.delete"))
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
                                tr!("server.name"),
                                true,
                                h_flex()
                                    .gap_4()
                                    .child(
                                        div()
                                            .flex_1()
                                            .child(Input::new(&self.name).aria_label(tr!("script.name"))),
                                    )
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
                                            .child(Radio::new("kind-before").label(tr!("script.before_publish")))
                                            .child(Radio::new("kind-after").label(tr!("script.after_receive"))),
                                    ),
                                cx,
                            ))
                            .child(form_row(
                                tr!("common.description"),
                                false,
                                Input::new(&self.description).aria_label(tr!("script.description")),
                                cx,
                            ))
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
                            .child(
                                div()
                                    .flex_1()
                                    .min_h(px(130.))
                                    .child(Editor::new(&self.code).aria_label(tr!("script.code")).h_full()),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .flex_1()
                                            .child(Input::new(&self.sample).aria_label(tr!("script.sample")).small()),
                                    )
                                    .child(
                                        Button::new("script-test")
                                            .outline()
                                            .small()
                                            .icon(IconName::Play)
                                            .label(tr!("script.test_run"))
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
                                Button::new("script-import")
                                    .outline()
                                    .icon(Lucide::FileInput)
                                    .label(tr!("script.import"))
                                    .on_click(move |_, window, cx| {
                                        e_import.update(cx, |this, cx| this.import_file(window, cx))
                                    }),
                            )
                            .child(div().flex_1())
                            .child(
                                Button::new("script-help")
                                    .outline()
                                    .icon(IconName::BookOpen)
                                    .label(tr!("script.view_functions"))
                                    .on_click(|_, window, cx| open_help(window, cx)),
                            )
                            .child(
                                Button::new("script-save")
                                    .primary()
                                    .icon(Lucide::Save)
                                    .label(tr!("common.save"))
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
            .title(tr!("script.title"))
            .w(px(1040.))
            .child(manager.clone())
            .footer(DialogFooter::new().child(super::common::close_button(tr!("common.close"), false)))
    });
}

/// Largest script file accepted by "Import".
const MAX_IMPORT_BYTES: u64 = 1024 * 1024;

fn read_script_file(path: &std::path::Path) -> Result<String, String> {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if !matches!(ext.as_str(), "js" | "mjs" | "txt") {
        return Err(tr!("script.import_wrong_type").into());
    }
    let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > MAX_IMPORT_BYTES {
        return Err(tr!("script.import_too_large").into());
    }
    let bytes = std::fs::read(path).map_err(|e| tr!("script.import_failed", error = e).to_string())?;
    let text = String::from_utf8(bytes).map_err(|_| tr!("script.import_not_utf8").to_string())?;
    // Drop a UTF-8 byte order mark written by some Windows editors.
    Ok(text.strip_prefix('\u{feff}').map(str::to_string).unwrap_or(text))
}

/// `(section key, [(signature, description key)])` of the function help;
/// the keys live under `script.help.` in the locale file.
const HELP: &[(&str, &[(&str, &str)])] = &[
    (
        "entry",
        &[
            ("process(payload, topic)", "process"),
            ("payload / topic", "globals"),
            ("console.log / warn / error", "console"),
            ("atob / btoa", "atob"),
            ("JSON, Math, Date, ...", "builtins"),
        ],
    ),
    (
        "env",
        &[
            ("env.NAME", "env_name"),
            ("env.get(\"NAME\")", "env_get"),
            ("env.replace(text)", "env_replace"),
            ("env.all()", "env_all"),
        ],
    ),
    (
        "encoding",
        &[
            ("crypto.stringToBytes(str)", "string_to_bytes"),
            ("crypto.bytesToString(bytes)", "bytes_to_string"),
            ("crypto.bytesToBase64(bytes) / base64ToBytes(str)", "base64"),
            ("crypto.bytesToHex(bytes) / hexToBytes(hex)", "hex"),
        ],
    ),
    (
        "hash",
        &[
            ("await crypto.sha256(data)", "sha256"),
            ("await crypto.sha1(data)", "sha1"),
            ("crypto.md5(data)", "md5"),
            ("await crypto.hmacSha256(key, data)", "hmac"),
            ("crypto.crc32(data)", "crc32"),
        ],
    ),
    (
        "aes_base64",
        &[
            ("await crypto.aesGcmEncrypt(text, keyBase64, ivBase64?)", "gcm_encrypt"),
            ("await crypto.aesGcmDecrypt(cipherBase64, keyBase64)", "gcm_decrypt"),
            ("await crypto.aesCbcEncrypt(text, keyBase64, ivBase64?)", "cbc_encrypt"),
            ("await crypto.aesCbcDecrypt(cipherBase64, keyBase64)", "cbc_decrypt"),
        ],
    ),
    (
        "aes_hex",
        &[
            (
                "await crypto.aesGcmEncryptHex(text, keyHex, ivHex?)",
                "gcm_encrypt_short",
            ),
            ("await crypto.aesGcmDecryptHex(cipherHex, keyHex)", "gcm_decrypt"),
            (
                "await crypto.aesCbcEncryptHex(text, keyHex, ivHex?)",
                "cbc_encrypt_short",
            ),
            ("await crypto.aesCbcDecryptHex(cipherHex, keyHex)", "cbc_decrypt"),
        ],
    ),
    (
        "other",
        &[
            ("crypto.randomBytes(length)", "random_bytes"),
            ("crypto.generateKey(bits = 128)", "generate_key"),
            ("crypto.generateIv(length = 12)", "generate_iv"),
            ("crypto.xor(data, key)", "xor"),
        ],
    ),
];

fn help_text(key: &str) -> SharedString {
    tr!(format!("script.help.{key}"))
}

fn open_help(window: &mut Window, cx: &mut App) {
    window.open_dialog(cx, |dialog, _, cx| {
        let theme = cx.theme().clone();
        dialog
            .title(tr!("script.functions"))
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
                            .child(div().text_sm().font_semibold().child(help_text(section)))
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
                                    .child(div().flex_1().text_color(theme.muted_foreground).child(help_text(desc)))
                            }))
                    }))
                    .child(
                        v_flex()
                            .gap_1()
                            .child(div().text_sm().font_semibold().child(tr!("script.example")))
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
            .footer(DialogFooter::new().child(super::common::close_button(tr!("script.got_it"), true)))
    });
}

#[cfg(test)]
mod tests {
    // Not `super::*`: that would bring GPUI's `test` attribute into scope.
    use super::{HELP, read_script_file};

    #[test]
    fn script_files_are_read_as_utf8_text() {
        let dir = std::env::temp_dir().join(format!("mini-mqtt-test-import-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let js = dir.join("a.JS");
        std::fs::write(&js, "\u{feff}function process(p) { return p; }").unwrap();
        assert_eq!(read_script_file(&js).unwrap(), "function process(p) { return p; }");
        let exe = dir.join("a.exe");
        std::fs::write(&exe, "x").unwrap();
        assert!(read_script_file(&exe).is_err());
        let bin = dir.join("b.txt");
        std::fs::write(&bin, [0xff, 0xfe, 0x00]).unwrap();
        assert!(read_script_file(&bin).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn help_texts_exist_in_both_languages() {
        let keys = HELP
            .iter()
            .flat_map(|(section, items)| std::iter::once(*section).chain(items.iter().map(|(_, d)| *d)));
        for key in keys {
            let id = format!("script.help.{key}");
            for locale in [crate::i18n::ZH_CN, crate::i18n::EN] {
                assert_ne!(rust_i18n::t!(&id, locale = locale), id, "{id} missing for {locale}");
            }
        }
    }
}
