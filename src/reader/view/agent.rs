use super::{super::Reader, AGENT_WIDTH, floating::field};
use crate::agent::{
    catalog::Availability,
    conversation::{Block, ChoiceKind, Entry, PlanStep, Status, ToolRow, blocks, page_references},
};
use crate::reader::{agent::Connection, chrome::Input};
use crate::ui::{
    kit::{RADIUS, clickable, dot, label},
    theme::Theme,
};
use gpui::{
    AnyElement, Context, FontWeight, HighlightStyle, Hsla, InteractiveText, IntoElement,
    MouseButton, StyledText, div, prelude::*, px,
};

const PLATFORM: &str = if cfg!(target_os = "macos") {
    "this Mac"
} else {
    "this PC"
};

fn quiet(
    id: impl Into<gpui::ElementId>,
    text: &str,
    color: Hsla,
    theme: &Theme,
) -> gpui::Stateful<gpui::Div> {
    let hover = theme.hover;
    clickable(id)
        .h(px(28.0))
        .px(px(10.0))
        .flex()
        .flex_none()
        .items_center()
        .rounded(px(RADIUS))
        .text_size(px(12.0))
        .text_color(color)
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .child(text.to_string())
}

fn primary(
    id: impl Into<gpui::ElementId>,
    text: &str,
    theme: &Theme,
    enabled: bool,
) -> gpui::Stateful<gpui::Div> {
    let strong = theme.accent_strong;
    clickable(id)
        .h(px(28.0))
        .px(px(12.0))
        .flex()
        .flex_none()
        .items_center()
        .rounded(px(RADIUS))
        .text_size(px(13.0))
        .font_weight(FontWeight::SEMIBOLD)
        .bg(theme.accent)
        .text_color(theme.on_accent)
        .when(enabled, |button| {
            button.cursor_pointer().hover(move |style| style.bg(strong))
        })
        .when(!enabled, |button| button.opacity(0.4))
        .child(text.to_string())
}

impl Reader {
    pub(in crate::reader) fn agent_panel(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.chrome.theme;
        let (state, led) = match &self.agent.connection {
            Connection::Idle => ("Not started", theme.faint),
            Connection::Detecting => ("Looking for agents", theme.warn),
            Connection::Starting => ("Starting", theme.warn),
            Connection::Ready => ("Ready", theme.ok),
            Connection::Working => ("Working", theme.warn),
            Connection::Failed(_) => ("Stopped", theme.fail),
        };
        let header = div()
            .h(px(40.0))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(8.0))
            .pl(px(14.0))
            .pr(px(8.0))
            .border_b_1()
            .border_color(theme.hairline)
            .child(dot(led, 7.0))
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.agent.name.clone()),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(theme.muted)
                    .child(state),
            )
            .child(div().flex_1())
            .when(self.agent.agents.is_some(), |header| {
                header.child(
                    quiet("agent-change", "Change agent", theme.muted, &theme).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.agent.choosing = !this.agent.choosing;
                            cx.notify();
                        }),
                    ),
                )
            });
        let feed = if self.agent.conversation.entries.is_empty() || self.agent.choosing {
            self.agent_start(cx)
        } else {
            self.agent_feed(cx)
        };
        div()
            .w(px(AGENT_WIDTH))
            .flex_shrink_0()
            .h_full()
            .flex()
            .flex_col()
            .bg(theme.chrome)
            .border_l_1()
            .border_color(theme.hairline)
            .text_size(px(13.0))
            .child(header)
            .child(feed)
            .child(self.agent_composer(cx))
    }

    fn agent_start(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.chrome.theme;
        let mut column = div()
            .id("agent-start")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px(px(16.0))
            .pt(px(30.0))
            .pb(px(20.0))
            .flex()
            .flex_col()
            .gap(px(14.0))
            .text_color(theme.muted);
        let Some(agents) = &self.agent.agents else {
            return column
                .child(format!("Looking for agents on {PLATFORM}."))
                .into_any_element();
        };
        if let Connection::Failed(message) = &self.agent.connection {
            column = column
                .child(
                    div()
                        .text_color(theme.fail)
                        .child(format!("{} stopped.", self.agent.name)),
                )
                .child(div().text_size(px(12.0)).child(message.clone()));
        }
        let installed = agents
            .iter()
            .any(|agent| agent.availability != Availability::NotInstalled);
        if !installed {
            return column
                .child(div().text_color(theme.ink).child("No agents found"))
                .child(div().text_size(px(12.0)).child(
                    "Install Claude Code, Codex, or Gemini CLI, then close and reopen this panel. Claude Code and Codex also need Node.js.",
                ))
                .into_any_element();
        }
        let selected = self.agent.selected;
        let connection = self.agent.connection.clone();
        let hover = theme.hover;
        let rows = agents.iter().enumerate().map(|(index, agent)| {
            let (status, color) = match &agent.availability {
                Availability::NotInstalled => ("Not installed", theme.faint),
                Availability::NeedsNode => ("Needs Node.js", theme.warn),
                Availability::Ready(_) if selected == Some(index) => match connection {
                    Connection::Ready | Connection::Working => ("Connected", theme.ok),
                    Connection::Starting => ("Starting", theme.warn),
                    Connection::Failed(_) => ("Stopped", theme.fail),
                    _ => ("Available", theme.muted),
                },
                Availability::Ready(_) => ("Available", theme.muted),
            };
            let startable = matches!(agent.availability, Availability::Ready(_));
            clickable(("agent-row", index))
                .flex()
                .items_baseline()
                .gap(px(10.0))
                .py(px(4.0))
                .px(px(8.0))
                .mx(px(-8.0))
                .rounded(px(RADIUS))
                .when(startable, |row| {
                    row.cursor_pointer()
                        .hover(move |style| style.bg(hover))
                        .on_click(cx.listener(move |this, _, _, cx| this.start_agent(index, cx)))
                })
                .child(
                    div()
                        .w(px(110.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.ink)
                        .child(agent.spec.name),
                )
                .child(div().text_size(px(12.0)).text_color(color).child(status))
        });
        column = column
            .child(label(&theme, format!("Agents found on {PLATFORM}")))
            .child(div().flex().flex_col().children(rows))
            .child(div().text_size(px(12.0)).child(
                "Uses your installed agent and its subscription. The agent reads this PDF through the reader's local tools and sends what it reads to its own model provider.",
            ));
        if self.agent.ready() && self.tabs.current().is_some() {
            column = column.child(label(&theme, "Try")).child(
                quiet(
                    "agent-suggest-summary",
                    "Summarize this document",
                    theme.accent_strong,
                    &theme,
                )
                .ml(px(-10.0))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.agent.choosing = false;
                    this.ask_agent(None, cx)
                })),
            );
        }
        column.into_any_element()
    }

    fn agent_feed(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.chrome.theme;
        let working = self.agent.connection == Connection::Working;
        let entries = self.agent.conversation.entries.clone();
        let last_answer = entries
            .iter()
            .rposition(|entry| matches!(entry, Entry::Answer(_)));
        let rows = entries
            .into_iter()
            .enumerate()
            .map(|(index, entry)| match entry {
                Entry::User { text, context } => div()
                    .flex()
                    .justify_end()
                    .child(
                        div()
                            .max_w(px(AGENT_WIDTH * 0.88))
                            .bg(theme.accent_soft)
                            .text_color(theme.ink)
                            .px(px(12.0))
                            .py(px(8.0))
                            .rounded(px(RADIUS))
                            .child(text)
                            .child(
                                div()
                                    .mt(px(4.0))
                                    .text_size(px(11.5))
                                    .text_color(theme.muted)
                                    .child(context),
                            ),
                    )
                    .into_any_element(),
                Entry::Plan(steps) => plan(&theme, &steps),
                Entry::Tools(rows) => tools(&theme, &rows),
                Entry::Answer(text) => {
                    let answer = self.answer(index, &text, cx);
                    let actions = (Some(index) == last_answer && !working).then(|| {
                        div().flex().ml(px(-10.0)).child(
                            quiet("agent-copy", "Copy", theme.muted, &theme)
                                .on_click(cx.listener(|this, _, _, cx| this.copy_answer(cx))),
                        )
                    });
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        .child(answer)
                        .children(actions)
                        .into_any_element()
                }
                Entry::Permission {
                    request,
                    title,
                    choices,
                    chosen,
                } => {
                    let mut ask = div()
                        .flex()
                        .flex_col()
                        .gap(px(8.0))
                        .pt(px(10.0))
                        .pb(px(4.0))
                        .border_t_2()
                        .border_color(if chosen.is_some() {
                            theme.hairline
                        } else {
                            theme.warn
                        })
                        .child(
                            div()
                                .text_color(theme.ink)
                                .child(format!("Allow {} to use {title}?", self.agent.name)),
                        );
                    ask = match chosen {
                        Some(chosen) => ask.child(
                            div()
                                .text_size(px(12.0))
                                .text_color(theme.muted)
                                .child(chosen),
                        ),
                        None => ask.child(div().flex().flex_wrap().gap(px(6.0)).children(
                            choices.into_iter().enumerate().map(|(position, choice)| {
                                let allow_once = choice.kind == ChoiceKind::AllowOnce;
                                let reply = (choice.id.clone(), choice.name.clone());
                                let button = if allow_once {
                                    primary(("agent-allow", position), &choice.name, &theme, true)
                                } else {
                                    quiet(
                                        ("agent-choice", position),
                                        &choice.name,
                                        theme.ink,
                                        &theme,
                                    )
                                    .bg(theme.hover)
                                };
                                button.on_click(cx.listener(move |this, _, _, cx| {
                                    this.answer_permission(request, Some(reply.clone()), cx)
                                }))
                            }),
                        )),
                    };
                    ask.into_any_element()
                }
                Entry::Notice(text) => div()
                    .text_size(px(12.0))
                    .text_color(theme.muted)
                    .child(text)
                    .into_any_element(),
            });
        div()
            .id("agent-feed")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px(px(16.0))
            .pt(px(14.0))
            .pb(px(20.0))
            .flex()
            .flex_col()
            .gap(px(14.0))
            .children(rows)
            .into_any_element()
    }

    fn answer(&self, index: usize, text: &str, cx: &mut Context<Self>) -> AnyElement {
        let theme = self.chrome.theme;
        let reader = cx.entity().downgrade();
        let parts = blocks(text)
            .into_iter()
            .enumerate()
            .map(|(position, block)| {
                let (content, heading, bullet) = match block {
                    Block::Heading(text) => (text, true, false),
                    Block::Bullet(text) => (text, false, true),
                    Block::Paragraph(text) => (text, false, false),
                };
                if heading {
                    return label(&theme, content).mt(px(6.0)).into_any_element();
                }
                let references = page_references(&content);
                let ranges: Vec<_> = references.iter().map(|(range, _)| range.clone()).collect();
                let pages: Vec<_> = references.iter().map(|(_, page)| *page).collect();
                let styled = StyledText::new(content).with_highlights(ranges.iter().map(|range| {
                    (
                        range.clone(),
                        HighlightStyle {
                            color: Some(theme.accent_strong),
                            ..Default::default()
                        },
                    )
                }));
                let reader = reader.clone();
                let text = InteractiveText::new(("agent-text", index * 1000 + position), styled)
                    .on_click(ranges, move |clicked, window, cx| {
                        if let Some(page) = pages.get(clicked).copied() {
                            let _ =
                                reader.update(cx, |this, cx| this.jump(page - 1, None, window, cx));
                        }
                    });
                div()
                    .flex()
                    .gap(px(8.0))
                    .text_color(theme.ink)
                    .when(bullet, |row| {
                        row.child(div().flex_none().text_color(theme.faint).child("•"))
                    })
                    .child(div().flex_1().min_w_0().child(text))
                    .into_any_element()
            });
        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .children(parts)
            .into_any_element()
    }

    fn agent_composer(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.chrome.theme;
        let focused = self.chrome.input == Input::Agent;
        let ready = self.agent.ready();
        let working = self.agent.connection == Connection::Working;
        let has_tab = self.tabs.current().is_some();
        let context = self.tabs.current().map_or_else(
            || "Open a PDF to ask about it".to_string(),
            |tab| format!("Context: {}", tab.title()),
        );
        let can_send = ready && has_tab && !self.agent.composer.text.trim().is_empty();
        let send = if working {
            primary("agent-stop", "Stop", &theme, true)
                .on_click(cx.listener(|this, _, _, cx| this.stop_agent(cx)))
        } else {
            primary("agent-send", "Send", &theme, can_send).when(can_send, |button| {
                button.on_click(cx.listener(|this, _, _, cx| {
                    let question = this.agent.composer.text.trim().to_string();
                    this.agent.composer.clear();
                    this.ask_agent(Some(question), cx)
                }))
            })
        };
        div()
            .flex_none()
            .border_t_1()
            .border_color(theme.hairline)
            .px(px(12.0))
            .pt(px(10.0))
            .pb(px(12.0))
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(
                clickable("agent-composer")
                    .min_h(px(54.0))
                    .bg(theme.canvas)
                    .rounded(px(RADIUS))
                    .px(px(10.0))
                    .py(px(8.0))
                    .cursor_text()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            this.chrome.input = Input::Agent;
                            this.focus.focus(window);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                    .child(field(
                        &theme,
                        &self.agent.composer,
                        "Ask about this document",
                        focused,
                    )),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .text_size(px(12.0))
                    .text_color(theme.muted)
                    .when(ready && has_tab, |row| {
                        row.child(
                            quiet(
                                "agent-summarize",
                                "Summarize document",
                                theme.accent_strong,
                                &theme,
                            )
                            .ml(px(-8.0))
                            .on_click(cx.listener(|this, _, _, cx| this.ask_agent(None, cx))),
                        )
                    })
                    .child(div().flex_1().min_w_0().truncate().child(context))
                    .child(send),
            )
    }
}

fn plan(theme: &Theme, steps: &[PlanStep]) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .children(steps.iter().map(|step| {
            let (marker, marker_color, text_color) = match step.status {
                Status::Done => ("ok", theme.ok, theme.faint),
                Status::Running => ("...", theme.warn, theme.ink),
                Status::Failed => ("x", theme.fail, theme.muted),
                Status::Pending => ("", theme.faint, theme.muted),
            };
            div()
                .flex()
                .items_baseline()
                .gap(px(8.0))
                .child(
                    div()
                        .w(px(14.0))
                        .flex_none()
                        .font_family(Theme::mono_font())
                        .text_size(px(11.0))
                        .text_color(marker_color)
                        .child(marker),
                )
                .child(div().text_color(text_color).child(step.text.clone()))
        }))
        .into_any_element()
}

fn tools(theme: &Theme, rows: &[ToolRow]) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .border_t_1()
        .border_color(theme.hairline)
        .children(rows.iter().map(|row| {
            let (status, color) = match row.status {
                Status::Done => ("Done", theme.ok),
                Status::Failed => ("Failed", theme.fail),
                Status::Pending | Status::Running => ("Running", theme.warn),
            };
            div()
                .flex()
                .items_baseline()
                .gap(px(8.0))
                .py(px(7.0))
                .border_b_1()
                .border_color(theme.hairline)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .child(
                            div()
                                .font_weight(FontWeight::MEDIUM)
                                .child(row.title.clone()),
                        )
                        .when(!row.detail.is_empty(), |cell| {
                            cell.child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(theme.muted)
                                    .child(row.detail.clone()),
                            )
                        }),
                )
                .child(
                    div()
                        .flex_none()
                        .text_size(px(11.5))
                        .text_color(color)
                        .child(status),
                )
        }))
        .into_any_element()
}
