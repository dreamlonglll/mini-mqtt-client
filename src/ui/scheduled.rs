//! Scheduled / periodic publishing of templates.
//!
//! [`Scheduler`] owns the running job so it continues when the dialog is
//! closed; the publish panel shows its progress and a stop button.

use super::common::{
    IconName, Lucide, card, chip, empty_state, form_row, input_value, notify, number_field, number_state,
};
use crate::backend::models::{CommandTemplate, PayloadFormat};
use crate::backend::payload;
use crate::state::{AppState, PublishDraft, ToastKind};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::InputState;
use gpui_kit::component::radio::{Radio, RadioGroup};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::component::{Disableable as _, StyledExt as _};
use gpui_kit::{prelude::*, *};
use std::collections::VecDeque;
use std::time::Duration;

const MAX_LOGS: usize = 200;

#[derive(Clone, Copy, Debug)]
pub struct ScheduleConfig {
    pub interval_ms: u64,
    pub round_interval_ms: u64,
    pub infinite: bool,
    pub loop_count: u32,
}

#[derive(Clone)]
pub struct LogLine {
    pub time: String,
    pub topic: String,
    pub detail: String,
    pub ok: bool,
}

#[derive(Default)]
pub struct Scheduler {
    state: Option<Entity<AppState>>,
    server_id: Option<i64>,
    running: bool,
    /// A run happened (finished or stopped) and its results are still shown.
    has_results: bool,
    round: u32,
    index: usize,
    total: usize,
    current: Option<String>,
    sent: usize,
    success: usize,
    failed: usize,
    logs: VecDeque<LogLine>,
    task: Option<Task<anyhow::Result<()>>>,
}

impl Scheduler {
    pub fn new(state: Entity<AppState>, _cx: &mut Context<Self>) -> Self {
        Self {
            state: Some(state),
            ..Default::default()
        }
    }

    pub fn is_running_for(&self, server_id: i64) -> bool {
        self.running && self.server_id == Some(server_id)
    }

    pub fn progress_label(&self) -> SharedString {
        tr!(
            "scheduled.progress_label",
            round = self.round,
            current = (self.index + 1).min(self.total),
            total = self.total,
            success = self.success,
            failed = self.failed
        )
    }

    pub fn start(
        &mut self,
        server_id: i64,
        commands: Vec<CommandTemplate>,
        config: ScheduleConfig,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = self.state.clone() else {
            return;
        };
        self.server_id = Some(server_id);
        self.running = true;
        self.has_results = true;
        self.round = 1;
        self.index = 0;
        self.total = commands.len();
        self.current = None;
        self.sent = 0;
        self.success = 0;
        self.failed = 0;
        self.logs.clear();

        let interval = Duration::from_millis(config.interval_ms.max(10));
        let round_interval = Duration::from_millis(config.round_interval_ms);
        self.task = Some(cx.spawn(async move |this, cx| {
            let mut round: u32 = 1;
            // Deadline-based pacing: time spent publishing counts towards the
            // interval, while consecutive sends stay at least `interval` apart.
            let mut next_at = std::time::Instant::now();
            'run: loop {
                for (ix, command) in commands.iter().enumerate() {
                    let draft = PublishDraft::from(command);
                    let publish = state.update(cx, |state, cx| state.publish(server_id, draft, cx));
                    let result = publish.await;
                    this.update(cx, |s, cx| {
                        s.index = ix;
                        s.round = round;
                        s.current = Some(command.name.clone());
                        s.record(command, result, cx);
                    })?;

                    let wait = if ix + 1 == commands.len() {
                        if !config.infinite && round >= config.loop_count {
                            break 'run;
                        }
                        round += 1;
                        if round_interval.is_zero() {
                            interval
                        } else {
                            round_interval
                        }
                    } else {
                        interval
                    };
                    next_at += wait;
                    let now = std::time::Instant::now();
                    if next_at > now {
                        cx.background_executor().timer(next_at - now).await;
                    } else {
                        // Late: send now, but never catch up with a burst —
                        // the interval is a minimum spacing devices may rely on.
                        next_at = now;
                    }
                }
            }
            this.update(cx, |s, cx| {
                // Leave `s.task` alone: this code runs inside that task, and
                // the finished handle is replaced by the next `start`.
                s.running = false;
                s.push_log(LogLine {
                    time: now(),
                    topic: "—".into(),
                    detail: tr!("scheduled.finished_log", count = s.sent).to_string(),
                    ok: true,
                });
                if let Some(state) = s.state.clone() {
                    state.update(cx, |state, cx| {
                        state.toast(ToastKind::Success, tr!("scheduled.finished"), cx)
                    });
                }
                cx.notify();
            })
        }));
        cx.notify();
    }

    pub fn stop(&mut self, cx: &mut Context<Self>) {
        if self.running {
            self.task = None; // dropping the task cancels it
            self.running = false;
            self.push_log(LogLine {
                time: now(),
                topic: "—".into(),
                detail: tr!("scheduled.stopped").to_string(),
                ok: false,
            });
            if let Some(state) = self.state.clone() {
                state.update(cx, |state, cx| {
                    state.toast(ToastKind::Info, tr!("scheduled.stopped"), cx)
                });
            }
            cx.notify();
        }
    }

    fn reset_results(&mut self, cx: &mut Context<Self>) {
        if !self.running {
            self.has_results = false;
            self.logs.clear();
            cx.notify();
        }
    }

    fn record(&mut self, command: &CommandTemplate, result: Result<(), String>, cx: &mut Context<Self>) {
        self.sent += 1;
        let (ok, detail) = match result {
            Ok(()) => {
                self.success += 1;
                (true, payload::truncate(&command.payload, 80))
            }
            Err(err) => {
                self.failed += 1;
                (false, err)
            }
        };
        self.push_log(LogLine {
            time: now(),
            topic: command.topic.clone(),
            detail,
            ok,
        });
        cx.notify();
    }

    fn push_log(&mut self, line: LogLine) {
        self.logs.push_front(line);
        self.logs.truncate(MAX_LOGS);
    }
}

fn now() -> String {
    chrono::Local::now().format("%H:%M:%S%.3f").to_string()
}

struct ScheduleDialog {
    state: Entity<AppState>,
    scheduler: Entity<Scheduler>,
    server_id: i64,
    /// Selected template ids, in click order.
    selected: Vec<i64>,
    interval: Entity<InputState>,
    round_interval: Entity<InputState>,
    loop_count: Entity<InputState>,
    order_by_name: bool,
    infinite: bool,
    _subscriptions: Vec<Subscription>,
}

impl ScheduleDialog {
    fn templates(&self, cx: &App) -> Vec<CommandTemplate> {
        self.state.read(cx).templates(self.server_id)
    }

    fn toggle(&mut self, id: i64, cx: &mut Context<Self>) {
        if let Some(pos) = self.selected.iter().position(|x| *x == id) {
            self.selected.remove(pos);
        } else {
            self.selected.push(id);
        }
        cx.notify();
    }

    fn start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let templates = self.templates(cx);
        let mut commands: Vec<CommandTemplate> = self
            .selected
            .iter()
            .filter_map(|id| templates.iter().find(|t| t.id == Some(*id)).cloned())
            .collect();
        if commands.is_empty() {
            notify(window, cx, ToastKind::Warning, tr!("scheduled.select_one"));
            return;
        }
        if !self.state.read(cx).mqtt.is_connected(self.server_id) {
            notify(window, cx, ToastKind::Warning, tr!("publish.connect_first"));
            return;
        }
        if self.order_by_name {
            commands.sort_by(|a, b| a.name.cmp(&b.name));
        }
        let parse = |s: &Entity<InputState>, cx: &App| input_value(s, cx).trim().parse::<u64>().unwrap_or(0);
        let config = ScheduleConfig {
            interval_ms: parse(&self.interval, cx).max(10),
            round_interval_ms: parse(&self.round_interval, cx),
            infinite: self.infinite,
            loop_count: parse(&self.loop_count, cx).clamp(1, u32::MAX as u64) as u32,
        };
        let server_id = self.server_id;
        self.scheduler
            .update(cx, |s, cx| s.start(server_id, commands, config, cx));
    }

    fn render_config(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let templates = self.templates(cx);
        let entity = cx.entity();
        let all_selected = !templates.is_empty() && self.selected.len() == templates.len();
        let all_ids: Vec<i64> = templates.iter().filter_map(|t| t.id).collect();
        let (e_all, e_order, e_loop, e_start) = (entity.clone(), entity.clone(), entity.clone(), entity.clone());

        v_flex()
            .gap_4()
            .child(
                card(cx)
                    .child(
                        h_flex()
                            .h(px(44.))
                            .px_4()
                            .justify_between()
                            .border_b_1()
                            .border_color(theme.border)
                            .child(div().font_semibold().child(tr!("scheduled.commands")))
                            .child(
                                h_flex()
                                    .gap_3()
                                    .child(
                                        Checkbox::new("select-all")
                                            .label(tr!("scheduled.select_all"))
                                            .checked(all_selected)
                                            .disabled(templates.is_empty())
                                            .on_click(move |checked, _, cx| {
                                                let checked = *checked;
                                                let ids = all_ids.clone();
                                                e_all.update(cx, |this, cx| {
                                                    this.selected = if checked { ids } else { Vec::new() };
                                                    cx.notify();
                                                })
                                            }),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.muted_foreground)
                                            .child(tr!("scheduled.selected", count = self.selected.len())),
                                    ),
                            ),
                    )
                    .child(
                        v_flex()
                            .id("schedule-templates")
                            .max_h(px(240.))
                            .overflow_y_scrollbar()
                            .when(templates.is_empty(), |this| {
                                this.child(div().h(px(140.)).child(empty_state(
                                    IconName::Inbox,
                                    tr!("scheduled.no_templates"),
                                    cx,
                                )))
                            })
                            .children(templates.iter().map(|t| {
                                let id = t.id.unwrap_or_default();
                                let order = self.selected.iter().position(|x| *x == id);
                                let entity = entity.clone();
                                let format_color = match t.format() {
                                    PayloadFormat::Json => theme.primary,
                                    PayloadFormat::Hex => theme.warning,
                                    PayloadFormat::Text => theme.muted_foreground,
                                };
                                h_flex()
                                    .id(("schedule-tpl", id as u64))
                                    .role(Role::ListItem)
                                    .aria_label(match order {
                                        Some(ix) => format!(
                                            "{} {} {} #{}",
                                            tr!("scheduled.command"),
                                            t.name,
                                            tr!("scheduled.chosen"),
                                            ix + 1
                                        ),
                                        None => format!("{} {}", tr!("scheduled.command"), t.name),
                                    })
                                    .px_4()
                                    .py_2()
                                    .gap_3()
                                    .cursor_pointer()
                                    .border_b_1()
                                    .border_color(theme.border.opacity(0.5))
                                    .when(order.is_some(), |this| this.bg(theme.accent.opacity(0.6)))
                                    .on_click(move |_, _, cx| entity.update(cx, |this, cx| this.toggle(id, cx)))
                                    .child(Checkbox::new(("schedule-check", id as u64)).checked(order.is_some()))
                                    .child(
                                        v_flex()
                                            .flex_1()
                                            .min_w_0()
                                            .gap_0p5()
                                            .child(div().text_sm().font_medium().child(t.name.clone()))
                                            .child(
                                                h_flex()
                                                    .gap_2()
                                                    .text_xs()
                                                    .text_color(theme.muted_foreground)
                                                    .child(
                                                        div()
                                                            .font_family(theme.mono_font_family.clone())
                                                            .child(t.topic.clone()),
                                                    )
                                                    .child(
                                                        div()
                                                            .min_w_0()
                                                            .truncate()
                                                            .child(payload::truncate(&t.payload, 50)),
                                                    ),
                                            ),
                                    )
                                    .child(chip(t.format().label(), format_color, cx))
                                    .child(chip(format!("QoS {}", t.qos), theme.primary, cx))
                                    .when_some(order, |this, ix| {
                                        this.child(chip(format!("#{}", ix + 1), theme.success, cx))
                                    })
                            })),
                    ),
            )
            .child(
                card(cx)
                    .child(
                        div()
                            .h(px(44.))
                            .px_4()
                            .flex()
                            .items_center()
                            .border_b_1()
                            .border_color(theme.border)
                            .font_semibold()
                            .child(tr!("scheduled.config")),
                    )
                    .child(
                        v_flex()
                            .p_4()
                            .gap_4()
                            .child(
                                h_flex()
                                    .gap_6()
                                    .child(form_row(
                                        tr!("scheduled.interval"),
                                        false,
                                        h_flex()
                                            .gap_2()
                                            .child(div().w(px(150.)).child(number_field(
                                                "interval",
                                                &tr!("scheduled.interval"),
                                                &self.interval,
                                                100,
                                                10,
                                                86_400_000,
                                            )))
                                            .child("ms"),
                                        cx,
                                    ))
                                    .child(form_row(
                                        tr!("scheduled.round_interval"),
                                        false,
                                        h_flex()
                                            .gap_2()
                                            .child(div().w(px(150.)).child(number_field(
                                                "round-interval",
                                                &tr!("scheduled.round_interval"),
                                                &self.round_interval,
                                                100,
                                                0,
                                                86_400_000,
                                            )))
                                            .child("ms"),
                                        cx,
                                    )),
                            )
                            .child(form_row(
                                tr!("scheduled.order"),
                                false,
                                div().pt_1p5().child(
                                    RadioGroup::horizontal("schedule-order")
                                        .selected_index(Some(if self.order_by_name { 1 } else { 0 }))
                                        .on_click(move |ix, _, cx| {
                                            let ix = *ix;
                                            e_order.update(cx, |this, cx| {
                                                this.order_by_name = ix == 1;
                                                cx.notify();
                                            })
                                        })
                                        .child(Radio::new("order-selection").label(tr!("scheduled.order_selection")))
                                        .child(Radio::new("order-name").label(tr!("scheduled.order_name"))),
                                ),
                                cx,
                            ))
                            .child(form_row(
                                tr!("scheduled.loop_mode"),
                                false,
                                h_flex()
                                    .gap_4()
                                    .child(
                                        RadioGroup::horizontal("schedule-loop")
                                            .selected_index(Some(if self.infinite { 0 } else { 1 }))
                                            .on_click(move |ix, _, cx| {
                                                let ix = *ix;
                                                e_loop.update(cx, |this, cx| {
                                                    this.infinite = ix == 0;
                                                    cx.notify();
                                                })
                                            })
                                            .child(Radio::new("loop-infinite").label(tr!("scheduled.infinite")))
                                            .child(Radio::new("loop-count").label(tr!("scheduled.count"))),
                                    )
                                    .when(!self.infinite, |this| {
                                        this.child(
                                            h_flex()
                                                .gap_2()
                                                .child(div().w(px(130.)).child(number_field(
                                                    "loop-count",
                                                    &tr!("scheduled.loop_count"),
                                                    &self.loop_count,
                                                    1,
                                                    1,
                                                    1_000_000,
                                                )))
                                                .child(tr!("scheduled.rounds_unit")),
                                        )
                                    }),
                                cx,
                            )),
                    ),
            )
            .child(
                h_flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("schedule-cancel")
                            .label(tr!("common.cancel"))
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("schedule-start")
                            .primary()
                            .icon(IconName::Play)
                            .label(tr!("scheduled.start"))
                            .on_click(move |_, window, cx| e_start.update(cx, |this, cx| this.start(window, cx))),
                    ),
            )
    }

    fn render_progress(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let s = self.scheduler.read(cx);
        let running = s.running;
        let stat = |label: SharedString, value: String, color: Hsla| {
            v_flex()
                .flex_1()
                .items_center()
                .py_3()
                .gap_1()
                .rounded(theme.radius)
                .bg(theme.muted.opacity(0.6))
                .child(div().text_xl().font_bold().text_color(color).child(value))
                .child(div().text_xs().text_color(theme.muted_foreground).child(label))
        };
        let stop = self.scheduler.clone();
        let back = self.scheduler.clone();

        v_flex()
            .id("schedule-progress")
            .role(Role::Status)
            .aria_label(tr!(
                "scheduled.status_aria",
                state = if running {
                    tr!("scheduled.running")
                } else {
                    tr!("scheduled.ended")
                },
                round = s.round,
                sent = s.sent,
                success = s.success,
                failed = s.failed
            ))
            .gap_4()
            .child(
                h_flex()
                    .gap_3()
                    .child(stat(
                        tr!("scheduled.current_round"),
                        s.round.to_string(),
                        theme.foreground,
                    ))
                    .child(stat(tr!("scheduled.sent"), s.sent.to_string(), theme.primary))
                    .child(stat(tr!("scheduled.success"), s.success.to_string(), theme.success))
                    .child(stat(tr!("scheduled.failed"), s.failed.to_string(), theme.danger)),
            )
            .child(
                h_flex()
                    .gap_2()
                    .text_sm()
                    .child(div().size(px(8.)).rounded_full().bg(if running {
                        theme.success
                    } else {
                        theme.muted_foreground
                    }))
                    .child(if running {
                        tr!(
                            "scheduled.sending",
                            name = s.current.clone().unwrap_or_default(),
                            current = (s.index + 1).min(s.total),
                            total = s.total
                        )
                    } else {
                        tr!("scheduled.ended")
                    }),
            )
            .child(
                card(cx).child(
                    v_flex()
                        .id("schedule-logs")
                        .h(px(300.))
                        .p_2()
                        .gap_0p5()
                        .overflow_y_scrollbar()
                        .text_xs()
                        .font_family(theme.mono_font_family.clone())
                        .children(s.logs.iter().map(|line| {
                            h_flex()
                                .gap_2()
                                .child(
                                    div()
                                        .flex_none()
                                        .text_color(theme.muted_foreground)
                                        .child(line.time.clone()),
                                )
                                .child(
                                    div()
                                        .flex_none()
                                        .text_color(if line.ok { theme.success } else { theme.danger })
                                        .child(if line.ok { "✓" } else { "✗" }),
                                )
                                .child(div().flex_none().child(line.topic.clone()))
                                .child(
                                    div()
                                        .min_w_0()
                                        .truncate()
                                        .text_color(theme.muted_foreground)
                                        .child(line.detail.clone()),
                                )
                        })),
                ),
            )
            .child(
                h_flex()
                    .justify_end()
                    .gap_2()
                    .when(running, |this| {
                        this.child(
                            Button::new("schedule-background")
                                .label(tr!("scheduled.background"))
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("schedule-stop")
                                .danger()
                                .icon(Lucide::CircleStop)
                                .label(tr!("scheduled.stop"))
                                .on_click(move |_, _, cx| stop.update(cx, |s, cx| s.stop(cx))),
                        )
                    })
                    .when(!running, |this| {
                        this.child(
                            Button::new("schedule-back")
                                .label(tr!("scheduled.back"))
                                .on_click(move |_, _, cx| back.update(cx, |s, cx| s.reset_results(cx))),
                        )
                        .child(
                            Button::new("schedule-close")
                                .primary()
                                .label(tr!("common.close"))
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                    }),
            )
    }
}

impl Render for ScheduleDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = self.scheduler.read(cx);
        let show_progress = s.server_id == Some(self.server_id) && (s.running || s.has_results);
        if show_progress {
            self.render_progress(cx).into_any_element()
        } else {
            self.render_config(cx).into_any_element()
        }
    }
}

pub fn open(scheduler: Entity<Scheduler>, state: Entity<AppState>, server_id: i64, window: &mut Window, cx: &mut App) {
    let other_running = {
        let s = scheduler.read(cx);
        s.running && s.server_id != Some(server_id)
    };
    if other_running {
        notify(window, cx, ToastKind::Warning, tr!("scheduled.other_running"));
        return;
    }
    let view = cx.new(|cx| {
        let subscriptions = vec![
            cx.observe(&scheduler, |_, _, cx| cx.notify()),
            cx.observe(&state, |_, _, cx| cx.notify()),
        ];
        ScheduleDialog {
            interval: number_state(window, cx, 1000),
            round_interval: number_state(window, cx, 0),
            loop_count: number_state(window, cx, 10),
            state,
            scheduler,
            server_id,
            selected: Vec::new(),
            order_by_name: false,
            infinite: true,
            _subscriptions: subscriptions,
        }
    });
    window.open_dialog(cx, move |dialog, _, _| {
        dialog.title(tr!("scheduled.title")).w(px(860.)).child(view.clone())
    });
}
