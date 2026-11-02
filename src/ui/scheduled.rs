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
/// After a short stall the scheduler may send up to this many intervals'
/// worth of messages back to back to keep the configured average rate.
const MAX_CATCH_UP: u32 = 5;

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
        format!(
            "第 {} 轮 · {}/{} · 成功 {} 失败 {}",
            self.round,
            (self.index + 1).min(self.total),
            self.total,
            self.success,
            self.failed
        )
        .into()
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
            // interval, so the configured rate is kept (as far as possible).
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
                    } else if now - next_at > wait * MAX_CATCH_UP {
                        // Far behind (e.g. the machine was busy): resume from
                        // now instead of bursting through the backlog.
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
                    detail: format!("定时发布完成，共发送 {} 条", s.sent),
                    ok: true,
                });
                if let Some(state) = s.state.clone() {
                    state.update(cx, |state, cx| state.toast(ToastKind::Success, "定时发布完成", cx));
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
                detail: "已停止定时发布".into(),
                ok: false,
            });
            if let Some(state) = self.state.clone() {
                state.update(cx, |state, cx| state.toast(ToastKind::Info, "已停止定时发布", cx));
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
            notify(window, cx, ToastKind::Warning, "请至少选择一条命令");
            return;
        }
        if !self.state.read(cx).mqtt.is_connected(self.server_id) {
            notify(window, cx, ToastKind::Warning, "请先连接服务器");
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
                            .child(div().font_semibold().child("命令列表"))
                            .child(
                                h_flex()
                                    .gap_3()
                                    .child(
                                        Checkbox::new("select-all")
                                            .label("全选")
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
                                            .child(format!("已选择 {} 条命令", self.selected.len())),
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
                                    "暂无模板，请先在「管理模板」中创建命令",
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
                                        Some(ix) => format!("命令 {} 已选 #{}", t.name, ix + 1),
                                        None => format!("命令 {}", t.name),
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
                            .child("发送配置"),
                    )
                    .child(
                        v_flex()
                            .p_4()
                            .gap_4()
                            .child(
                                h_flex()
                                    .gap_6()
                                    .child(form_row(
                                        "发送间隔",
                                        false,
                                        h_flex()
                                            .gap_2()
                                            .child(div().w(px(150.)).child(number_field(
                                                "interval",
                                                "发送间隔",
                                                &self.interval,
                                                100,
                                                10,
                                                86_400_000,
                                            )))
                                            .child("ms"),
                                        cx,
                                    ))
                                    .child(form_row(
                                        "每轮间隔",
                                        false,
                                        h_flex()
                                            .gap_2()
                                            .child(div().w(px(150.)).child(number_field(
                                                "round-interval",
                                                "每轮间隔",
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
                                "发送顺序",
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
                                        .child(Radio::new("order-selection").label("按勾选顺序"))
                                        .child(Radio::new("order-name").label("按名称排序")),
                                ),
                                cx,
                            ))
                            .child(form_row(
                                "循环模式",
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
                                            .child(Radio::new("loop-infinite").label("无限循环"))
                                            .child(Radio::new("loop-count").label("指定次数")),
                                    )
                                    .when(!self.infinite, |this| {
                                        this.child(
                                            h_flex()
                                                .gap_2()
                                                .child(div().w(px(130.)).child(number_field(
                                                    "loop-count",
                                                    "循环次数",
                                                    &self.loop_count,
                                                    1,
                                                    1,
                                                    1_000_000,
                                                )))
                                                .child("轮"),
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
                            .label("取消")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("schedule-start")
                            .primary()
                            .icon(IconName::Play)
                            .label("开始发布")
                            .on_click(move |_, window, cx| e_start.update(cx, |this, cx| this.start(window, cx))),
                    ),
            )
    }

    fn render_progress(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let s = self.scheduler.read(cx);
        let running = s.running;
        let stat = |label: &'static str, value: String, color: Hsla| {
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
            .aria_label(format!(
                "定时发布 {} 轮次 {} 已发送 {} 成功 {} 失败 {}",
                if running { "运行中" } else { "已结束" },
                s.round,
                s.sent,
                s.success,
                s.failed
            ))
            .gap_4()
            .child(
                h_flex()
                    .gap_3()
                    .child(stat("当前轮次", s.round.to_string(), theme.foreground))
                    .child(stat("已发送", s.sent.to_string(), theme.primary))
                    .child(stat("成功", s.success.to_string(), theme.success))
                    .child(stat("失败", s.failed.to_string(), theme.danger)),
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
                        format!(
                            "正在发送：{}（{}/{}）",
                            s.current.clone().unwrap_or_default(),
                            (s.index + 1).min(s.total),
                            s.total
                        )
                    } else {
                        "已结束".to_string()
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
                                .label("后台运行")
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("schedule-stop")
                                .danger()
                                .icon(Lucide::CircleStop)
                                .label("停止")
                                .on_click(move |_, _, cx| stop.update(cx, |s, cx| s.stop(cx))),
                        )
                    })
                    .when(!running, |this| {
                        this.child(
                            Button::new("schedule-back")
                                .label("返回配置")
                                .on_click(move |_, _, cx| back.update(cx, |s, cx| s.reset_results(cx))),
                        )
                        .child(
                            Button::new("schedule-close")
                                .primary()
                                .label("关闭")
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
        notify(window, cx, ToastKind::Warning, "其他服务器正在定时发布，请先停止");
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
        dialog.title("定时发布").w(px(860.)).child(view.clone())
    });
}
