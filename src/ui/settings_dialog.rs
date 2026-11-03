//! System settings: theme, language, data location, logs.

use super::common::{IconName, Lucide, confirm, notify, section_label};
use crate::backend::config::ThemePreference;
use crate::i18n::Language;
use crate::state::{AppEvent, AppState, ToastKind};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::dialog::DialogFooter;
use gpui_kit::component::input::{Editor, EditorState};
use gpui_kit::component::radio::{Radio, RadioGroup};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::{prelude::*, *};
use std::path::PathBuf;

struct SettingsView {
    state: Entity<AppState>,
    pending_path: Option<PathBuf>,
    migrate: bool,
    logs: Entity<EditorState>,
    _subscriptions: Vec<Subscription>,
}

impl SettingsView {
    fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let logs = cx.new(|cx| EditorState::new(window, cx).line_number(false).soft_wrap(true));
        let subscriptions = vec![
            cx.observe(&state, |_, _, cx| cx.notify()),
            // The "no errors" placeholder text lives in the editor.
            cx.subscribe_in(&state, window, |this, _, event: &AppEvent, window, cx| {
                if matches!(event, AppEvent::LanguageChanged) {
                    this.reload_logs(window, cx);
                }
            }),
        ];
        let mut this = Self {
            state,
            pending_path: None,
            migrate: true,
            logs,
            _subscriptions: subscriptions,
        };
        this.reload_logs(window, cx);
        this
    }

    fn reload_logs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let lines = self.state.read(cx).logs.recent_logs(100);
        let text = if lines.is_empty() {
            tr!("settings.no_logs").to_string()
        } else {
            lines.join("\n")
        };
        self.logs.update(cx, |s, cx| s.set_value(text, window, cx));
    }

    fn choose_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(tr!("settings.choose_folder")),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = paths.await
                && let Some(dir) = paths.into_iter().next()
            {
                let _ = this.update(cx, |this, cx| {
                    this.pending_path = Some(dir.join("data.yaml"));
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn apply_path(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.pending_path.clone() else {
            return;
        };
        let migrate = self.migrate;
        match self
            .state
            .update(cx, |state, cx| state.migrate_data_path(path, migrate, cx))
        {
            Ok(()) => {
                self.pending_path = None;
                notify(
                    window,
                    cx,
                    ToastKind::Success,
                    if migrate {
                        tr!("settings.migrated")
                    } else {
                        tr!("settings.path_changed")
                    },
                );
            }
            Err(err) => notify(window, cx, ToastKind::Error, tr!("settings.save_failed", error = err)),
        }
        cx.notify();
    }
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let state = self.state.read(cx);
        let theme_ix = match state.config.theme {
            ThemePreference::System => 0,
            ThemePreference::Light => 1,
            ThemePreference::Dark => 2,
        };
        let language_ix = match state.language() {
            Language::System => 0,
            Language::Chinese => 1,
            Language::English => 2,
        };
        let data_file = state.data_file();
        let data_dir = data_file.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let log_dir = state.logs.log_dir().to_path_buf();
        let entity = cx.entity();
        let (e_choose, e_apply, e_migrate, e_refresh) =
            (entity.clone(), entity.clone(), entity.clone(), entity.clone());
        let theme_state = self.state.clone();
        let language_state = self.state.clone();
        let clear_state = self.state.clone();
        let copy_path = data_file.to_string_lossy().to_string();

        let path_box = |id: &'static str, label: SharedString, path: String| {
            div()
                .id(id)
                .role(Role::Status)
                .aria_label(format!("{label} {path}"))
                .px_2()
                .py_1p5()
                .rounded(theme.radius)
                .bg(theme.muted)
                .text_xs()
                .font_family(theme.mono_font_family.clone())
                .child(path)
        };

        v_flex()
            .gap_5()
            .child(
                v_flex()
                    .gap_2()
                    .child(section_label(tr!("settings.appearance"), cx))
                    .child(
                        RadioGroup::horizontal("settings-theme")
                            .selected_index(Some(theme_ix))
                            .on_click(move |ix, _, cx| {
                                let pref = match *ix {
                                    1 => ThemePreference::Light,
                                    2 => ThemePreference::Dark,
                                    _ => ThemePreference::System,
                                };
                                theme_state.update(cx, |state, cx| state.set_theme(pref, cx))
                            })
                            .child(Radio::new("theme-system").label(tr!("settings.follow_system")))
                            .child(Radio::new("theme-light").label(tr!("settings.light")))
                            .child(Radio::new("theme-dark").label(tr!("settings.dark"))),
                    ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(section_label(tr!("settings.language"), cx))
                    .child(
                        RadioGroup::horizontal("settings-language")
                            .selected_index(Some(language_ix))
                            .on_click(move |ix, _, cx| {
                                let language = match *ix {
                                    1 => Language::Chinese,
                                    2 => Language::English,
                                    _ => Language::System,
                                };
                                language_state.update(cx, |state, cx| state.set_language(language, cx))
                            })
                            // Language names are shown in their own language.
                            .child(Radio::new("language-system").label(tr!("settings.follow_system")))
                            .child(Radio::new("language-zh").label("简体中文"))
                            .child(Radio::new("language-en").label("English")),
                    ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(section_label(tr!("settings.storage"), cx))
                    .child(path_box(
                        "data-path",
                        tr!("settings.data_file"),
                        data_file.to_string_lossy().to_string(),
                    ))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("data-change")
                                    .small()
                                    .icon(IconName::FolderOpen)
                                    .label(tr!("settings.change_location"))
                                    .on_click(move |_, window, cx| {
                                        e_choose.update(cx, |this, cx| this.choose_folder(window, cx))
                                    }),
                            )
                            .child(
                                Button::new("data-copy")
                                    .small()
                                    .icon(IconName::Copy)
                                    .label(tr!("settings.copy_path"))
                                    .on_click(move |_, window, cx| {
                                        cx.write_to_clipboard(ClipboardItem::new_string(copy_path.clone()));
                                        notify(window, cx, ToastKind::Success, tr!("settings.path_copied"));
                                    }),
                            )
                            .child(
                                Button::new("data-open")
                                    .small()
                                    .icon(IconName::ExternalLink)
                                    .label(tr!("settings.open_folder"))
                                    .on_click(move |_, _, cx| cx.open_with_system(&data_dir)),
                            ),
                    )
                    .when_some(self.pending_path.clone(), |this, path| {
                        this.child(
                            v_flex()
                                .gap_2()
                                .p_3()
                                .rounded(theme.radius)
                                .border_1()
                                .border_color(theme.warning.opacity(0.5))
                                .bg(theme.warning.opacity(0.06))
                                .child(div().text_sm().child(format!("{}:", tr!("settings.new_location"))))
                                .child(path_box(
                                    "pending-path",
                                    tr!("settings.new_location"),
                                    path.to_string_lossy().to_string(),
                                ))
                                .child(
                                    h_flex()
                                        .justify_between()
                                        .child(
                                            Checkbox::new("data-migrate")
                                                .label(tr!("settings.migrate_data"))
                                                .checked(self.migrate)
                                                .on_click(move |checked, _, cx| {
                                                    let checked = *checked;
                                                    e_migrate.update(cx, |this, cx| {
                                                        this.migrate = checked;
                                                        cx.notify();
                                                    })
                                                }),
                                        )
                                        .child(
                                            Button::new("data-apply")
                                                .primary()
                                                .small()
                                                .label(tr!("settings.apply_restart"))
                                                .on_click(move |_, window, cx| {
                                                    e_apply.update(cx, |this, cx| this.apply_path(window, cx))
                                                }),
                                        ),
                                ),
                        )
                    }),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(section_label(tr!("settings.logs"), cx))
                    .child(path_box(
                        "log-dir",
                        tr!("settings.log_dir"),
                        log_dir.to_string_lossy().to_string(),
                    ))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("logs-open")
                                    .small()
                                    .icon(IconName::FolderOpen)
                                    .label(tr!("settings.open_log_dir"))
                                    .on_click(move |_, _, cx| cx.open_with_system(&log_dir)),
                            )
                            .child(
                                Button::new("logs-refresh")
                                    .small()
                                    .icon(IconName::RefreshCw)
                                    .label(tr!("settings.refresh"))
                                    .on_click(move |_, window, cx| {
                                        e_refresh.update(cx, |this, cx| this.reload_logs(window, cx))
                                    }),
                            )
                            .child(
                                Button::new("logs-clear")
                                    .small()
                                    .danger()
                                    .icon(Lucide::Trash)
                                    .label(tr!("settings.clear_logs"))
                                    .on_click(move |_, window, cx| {
                                        let clear_state = clear_state.clone();
                                        let view = entity.clone();
                                        confirm(
                                            window,
                                            cx,
                                            tr!("settings.clear_logs"),
                                            tr!("settings.clear_logs_confirm"),
                                            tr!("common.clear"),
                                            move |window, cx| {
                                                let result = clear_state.read(cx).logs.clear();
                                                match result {
                                                    Ok(()) => notify(
                                                        window,
                                                        cx,
                                                        ToastKind::Success,
                                                        tr!("settings.logs_cleared"),
                                                    ),
                                                    Err(err) => notify(
                                                        window,
                                                        cx,
                                                        ToastKind::Error,
                                                        tr!("settings.clear_logs_failed", error = err),
                                                    ),
                                                }
                                                view.update(cx, |this, cx| this.reload_logs(window, cx));
                                            },
                                        );
                                    }),
                            ),
                    )
                    .child(
                        div().h(px(150.)).child(
                            Editor::new(&self.logs)
                                .aria_label(tr!("settings.recent_logs"))
                                .h_full()
                                .readonly(true),
                        ),
                    ),
            )
            .child(div().text_xs().text_color(theme.muted_foreground).child(format!(
                "Mini MQTT Client v{} · Rust + GPUI Kit",
                env!("CARGO_PKG_VERSION")
            )))
    }
}

pub fn open(state: Entity<AppState>, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| SettingsView::new(state, window, cx));
    window.open_dialog(cx, move |dialog, _, _| {
        dialog
            .title(tr!("settings.title"))
            .w(px(640.))
            .child(view.clone())
            .footer(DialogFooter::new().child(super::common::close_button(tr!("settings.done"), true)))
    });
}
