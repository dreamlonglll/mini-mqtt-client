//! Publish panel: topic, QoS, retain, payload format and the payload editor.

use super::common::{IconName, Lucide, card, card_header, input_value, notify, segmented};
use super::scheduled::{self, Scheduler};
use super::{template_dialog, template_drawer};
use crate::backend::models::{PayloadFormat, TemplateDraft};
use crate::backend::payload;
use crate::state::{AppEvent, AppState, PublishDraft, ToastKind};
use gpui_kit::component::Disableable as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Editor, EditorState, Input, InputState};
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::{prelude::*, *};

fn placeholder(format: PayloadFormat) -> SharedString {
    match format {
        PayloadFormat::Json => tr!("publish.placeholder_json"),
        PayloadFormat::Hex => tr!("publish.placeholder_hex"),
        PayloadFormat::Text => tr!("publish.placeholder_text"),
    }
}

pub struct PublishPanel {
    state: Entity<AppState>,
    scheduler: Entity<Scheduler>,
    topic: Entity<InputState>,
    payload: Entity<EditorState>,
    format: PayloadFormat,
    qos: u8,
    retain: bool,
    publishing: bool,
    _subscriptions: Vec<Subscription>,
}

impl PublishPanel {
    pub fn new(
        state: Entity<AppState>,
        scheduler: Entity<Scheduler>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let topic = cx.new(|cx| InputState::new(window, cx).placeholder(tr!("publish.topic_placeholder")));
        let format = PayloadFormat::Json;
        let payload = cx.new(|cx| {
            EditorState::new(window, cx)
                .line_number(false)
                .soft_wrap(true)
                .placeholder(placeholder(format))
        });
        let subscriptions = vec![
            cx.subscribe_in(&state, window, |this, _, event: &AppEvent, window, cx| match event {
                AppEvent::FillPublish(draft) => this.fill(draft.clone(), window, cx),
                AppEvent::LanguageChanged => {
                    this.topic.update(cx, |s, cx| {
                        s.set_placeholder(tr!("publish.topic_placeholder"), window, cx)
                    });
                    let format = this.format;
                    this.set_format(format, window, cx);
                }
                _ => {}
            }),
            cx.observe(&scheduler, |_, _, cx| cx.notify()),
        ];
        Self {
            state,
            scheduler,
            topic,
            payload,
            format,
            qos: 0,
            retain: false,
            publishing: false,
            _subscriptions: subscriptions,
        }
    }

    fn fill(&mut self, draft: PublishDraft, window: &mut Window, cx: &mut Context<Self>) {
        self.topic.update(cx, |s, cx| s.set_value(draft.topic, window, cx));
        self.payload.update(cx, |s, cx| s.set_value(draft.payload, window, cx));
        self.qos = draft.qos.min(2);
        self.retain = draft.retain;
        self.set_format(draft.format, window, cx);
    }

    fn set_format(&mut self, format: PayloadFormat, window: &mut Window, cx: &mut Context<Self>) {
        self.format = format;
        self.payload
            .update(cx, |s, cx| s.set_placeholder(placeholder(format), window, cx));
        cx.notify();
    }

    fn draft(&self, cx: &App) -> PublishDraft {
        PublishDraft {
            topic: input_value(&self.topic, cx).trim().to_string(),
            payload: self.payload.read(cx).value().to_string(),
            format: self.format,
            qos: self.qos,
            retain: self.retain,
        }
    }

    fn publish(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(server_id) = self.state.read(cx).active_server_id else {
            notify(window, cx, ToastKind::Warning, tr!("publish.select_server_first"));
            return;
        };
        if self.publishing {
            return;
        }
        let draft = self.draft(cx);
        let task = self.state.update(cx, |state, cx| state.publish(server_id, draft, cx));
        self.publishing = true;
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.publishing = false;
                match result {
                    Ok(()) => notify(window, cx, ToastKind::Success, tr!("publish.published")),
                    Err(err) => notify(window, cx, ToastKind::Error, err),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn format_json(&mut self, pretty: bool, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.payload.read(cx).value().to_string();
        let formatted = if pretty {
            payload::pretty_json(&text)
        } else {
            payload::compact_json(&text)
        };
        match formatted {
            Some(f) => self.payload.update(cx, |s, cx| s.set_value(f, window, cx)),
            None => notify(window, cx, ToastKind::Warning, tr!("payload.json_invalid_short")),
        }
    }

    fn save_as_template(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(server_id) = self.state.read(cx).active_server_id else {
            return;
        };
        let d = self.draft(cx);
        if d.topic.is_empty() {
            notify(window, cx, ToastKind::Warning, tr!("publish.enter_topic_first"));
            return;
        }
        let draft = TemplateDraft {
            name: String::new(),
            topic: d.topic,
            payload: d.payload,
            payload_type: d.format,
            qos: d.qos as i32,
            retain: d.retain,
            description: None,
            category: None,
        };
        template_dialog::open(self.state.clone(), server_id, None, draft, window, cx);
    }
}

impl Render for PublishPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let entity = cx.entity();
        let state = self.state.read(cx);
        let server_id = state.active_server_id.unwrap_or_default();
        let connected = state.status(server_id).is_connected();
        let format_ix = PayloadFormat::ALL.iter().position(|f| *f == self.format).unwrap_or(0);
        let scheduler = self.scheduler.read(cx);
        let schedule_running = scheduler.is_running_for(server_id);
        let schedule_progress = scheduler.progress_label();

        let e_format = entity.clone();
        let e_json = entity.clone();
        let e_qos = entity.clone();
        let e_retain = entity.clone();
        let e_save = entity.clone();
        let e_publish = entity.clone();
        let open_state = self.state.clone();
        let sched_state = self.state.clone();
        let sched = self.scheduler.clone();
        let stop_sched = self.scheduler.clone();

        card(cx)
            .child(
                card_header(Lucide::Send, tr!("publish.title"), cx)
                    .child(div().flex_1())
                    .when(self.format == PayloadFormat::Json, |this| {
                        this.child(
                            Button::new("format-json")
                                .ghost()
                                .small()
                                .icon(Lucide::Braces)
                                .label(tr!("publish.format_json"))
                                .on_click(move |_, window, cx| {
                                    e_json.update(cx, |this, cx| this.format_json(true, window, cx))
                                }),
                        )
                    })
                    .child(segmented(
                        "payload-format",
                        &tr!("publish.format_group"),
                        &["JSON", "TEXT", "HEX"],
                        format_ix,
                        move |ix, window, cx| {
                            e_format.update(cx, |this, cx| this.set_format(PayloadFormat::ALL[ix], window, cx))
                        },
                    ))
                    .when(schedule_running, |this| {
                        this.child(
                            h_flex()
                                .id("schedule-indicator")
                                .role(Role::Status)
                                .aria_label(format!("{} {schedule_progress}", tr!("scheduled.progress")))
                                .gap_1()
                                .px_2()
                                .rounded(theme.radius)
                                .bg(theme.success.opacity(0.1))
                                .text_xs()
                                .text_color(theme.success)
                                .child(Icon::new(Lucide::Timer).xsmall())
                                .child(schedule_progress.clone()),
                        )
                        .child(
                            Button::new("stop-schedule")
                                .danger()
                                .small()
                                .icon(Lucide::CircleStop)
                                .label(tr!("scheduled.stop"))
                                .on_click(move |_, _, cx| stop_sched.update(cx, |s, cx| s.stop(cx))),
                        )
                    })
                    .child(
                        Button::new("open-schedule")
                            .outline()
                            .small()
                            .icon(Lucide::Timer)
                            .label(tr!("scheduled.title"))
                            .on_click(move |_, window, cx| {
                                if let Some(id) = sched_state.read(cx).active_server_id {
                                    scheduled::open(sched.clone(), sched_state.clone(), id, window, cx);
                                }
                            }),
                    ),
            )
            .child(
                v_flex()
                    .p_3()
                    .gap_2()
                    .child(
                        h_flex()
                            .gap_3()
                            .child(
                                div().flex_1().child(
                                    Input::new(&self.topic)
                                        .aria_label(tr!("publish.topic"))
                                        .prefix(Icon::new(Lucide::Send).small().text_color(theme.muted_foreground)),
                                ),
                            )
                            .child(segmented(
                                "publish-qos",
                                &tr!("publish.qos"),
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
                                Checkbox::new("publish-retain")
                                    .label("Retain")
                                    .checked(self.retain)
                                    .on_click(move |checked, _, cx| {
                                        let checked = *checked;
                                        e_retain.update(cx, |this, cx| {
                                            this.retain = checked;
                                            cx.notify();
                                        })
                                    }),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_3()
                            .items_end()
                            .child(
                                div().flex_1().min_w_0().child(
                                    Editor::new(&self.payload)
                                        .aria_label(tr!("publish.payload"))
                                        .h(px(104.)),
                                ),
                            )
                            .child(
                                v_flex()
                                    .gap_2()
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .child(
                                                Button::new("open-templates")
                                                    .accessibility_label(tr!("publish.load_template"))
                                                    .outline()
                                                    .icon(IconName::FolderOpen)
                                                    .tooltip(tr!("publish.load_template"))
                                                    .on_click(move |_, window, cx| {
                                                        if let Some(id) = open_state.read(cx).active_server_id {
                                                            template_drawer::open(open_state.clone(), id, window, cx);
                                                        }
                                                    }),
                                            )
                                            .child(
                                                Button::new("save-template")
                                                    .accessibility_label(tr!("publish.save_template"))
                                                    .outline()
                                                    .icon(IconName::Star)
                                                    .tooltip(tr!("publish.save_template"))
                                                    .on_click(move |_, window, cx| {
                                                        e_save.update(cx, |this, cx| this.save_as_template(window, cx))
                                                    }),
                                            ),
                                    )
                                    .child(
                                        Button::new("publish")
                                            .primary()
                                            .w(px(108.))
                                            .icon(Lucide::Send)
                                            .label(tr!("publish.publish"))
                                            .loading(self.publishing)
                                            .disabled(!connected)
                                            .tooltip(if connected {
                                                tr!("publish.title")
                                            } else {
                                                tr!("publish.connect_first")
                                            })
                                            .on_click(move |_, window, cx| {
                                                e_publish.update(cx, |this, cx| this.publish(window, cx))
                                            }),
                                    ),
                            ),
                    ),
            )
    }
}
