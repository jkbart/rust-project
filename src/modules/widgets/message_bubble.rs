use ratatui::style::Modifier;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use std::sync::Arc;
use std::sync::Mutex;

use humansize::{format_size, DECIMAL};
use unicode_width::UnicodeWidthStr;

use crate::modules::protocol::*;
use crate::modules::widgets::list_component::*;

pub enum MsgBubbleAllignment {
    Left,
    Right,
}

pub struct LoadingBarStatus {
    pub position: u64,
    pub end: u64,
    pub changed: bool,
}

pub enum LoadingBar {
    Status(LoadingBarStatus),
    Error(String),
}

pub struct LoadingBarWrap {
    loadingbar: LoadingBar,
    changed: bool,
}

pub struct MsgBubble<'a> {
    pub received_from: Option<String>,
    pub message: UserMessage,
    pub loading_bar: Option<Arc<Mutex<LoadingBarWrap>>>, // Used for file downloading.
    allignment: MsgBubbleAllignment,
    render_cache: Option<ListCache<'a>>,
}

impl MsgBubble<'_> {
    pub fn new(
        received_from: Option<String>,
        message: UserMessage,
        allignment: MsgBubbleAllignment,
    ) -> Self {
        MsgBubble {
            received_from,
            message,
            loading_bar: None,
            allignment,
            render_cache: None,
        }
    }
}

impl<'a> ListItem<'a> for MsgBubble<'a> {
    fn get_cache(&mut self) -> &mut Option<ListCache<'a>> {
        if self
            .loading_bar
            .as_ref()
            .is_some_and(|lb| lb.lock().unwrap().changed)
        {
            self.render_cache = None;
        }

        &mut self.render_cache
    }

    fn prerender(&mut self, window_max_width: u16, selected: bool) {
        if let Some(loading_bar) = self.loading_bar.as_mut() {
            loading_bar.lock().unwrap().changed = false;
        }

        let style = if selected {
            Style::default().bg(Color::DarkGray) // Change background color if selected
        } else {
            Style::default()
        };

        let sender = self.received_from.as_deref().unwrap_or("You");

        // Length of name
        let name_length = UnicodeWidthStr::width(sender).min(window_max_width as usize - 2);

        // Total length of bubble. Will be only increased.
        let mut bubble_width = (name_length + 2).min(window_max_width as usize);

        let mut middle_lines: Vec<Vec<Span<'a>>> = Self::formatted_content(
            &self.message,
            &self.loading_bar,
            style.clone(),
            window_max_width - 4,
            &mut bubble_width,
        );

        // +/- 2/4 to bubble_width are related to adding "│ " " │"

        let top_line: Span<'a> = Span::styled(match self.allignment {
                MsgBubbleAllignment::Left => format!(
                    "┌{:─<width$}┐",
                    &sender[..name_length],
                    width = bubble_width + 2
                ),
                MsgBubbleAllignment::Right => format!(
                    "┌{:─>width$}┐",
                    &sender[..name_length],
                    width = bubble_width + 2
                ),
            },
            style.clone(),
        );

        let bot_line: Span<'a> = Span::styled(
            format!("└{}┘", "─".repeat(bubble_width + 2)),
            style.clone(),
        );

        for mid_line in middle_lines.iter_mut() {
            mid_line.insert(0, Span::styled("│ ", style.clone()));
            mid_line.push(Span::styled(" │", style.clone()));
        }

        let mut rendered_lines: Vec<Line<'a>> = vec![Line::from(top_line)];

        for line in middle_lines.into_iter() {
            rendered_lines.push(Line::from(line));
        }

        rendered_lines.push(Line::from(bot_line));

        let height = rendered_lines.len() as u16;

        self.render_cache = Some(ListCache::new(
            rendered_lines,
            window_max_width,
            height,
            selected,
        ));
    }
}

impl<'a> MsgBubble<'a> {
    fn formatted_content(
        message: &'a UserMessage,
        loading_bar: &'a Option<Arc<Mutex<LoadingBarWrap>>>,
        parent_style: Style,
        window_max_width: u16,
        bubble_width: &mut usize,
    ) -> Vec<Vec<Span<'a>>> {
        match &message {
            UserMessage::Text(text) => {
                // Little extra padding, because for text:
                // "C:\Users\jbart\AppData\Local\Packages\Microsoft.WindowsFeedbackHub_8wekyb3d8bbwe\LocalState\{18c6d0aa-02b6-4df0-982c-40fd41c34137}\Capture0.png"
                // textwrap::wrap returned one line longer than was asked. This is a problem with textwrap cargo.
                let mut lines = textwrap::wrap(&text, window_max_width as usize - 5);

                // Ensure that lines are not empty.
                if lines.is_empty() {
                    lines.push(std::borrow::Cow::Borrowed(" "));
                }

                let max_line_width = lines
                    .iter()
                    .map(|line| UnicodeWidthStr::width(line.as_ref()) as u16)
                    .max()
                    .unwrap_or(0)
                    .max(*bubble_width as u16);

                *bubble_width = max_line_width as usize;

                lines
                    .into_iter()
                    .map(|line| {
                            let line_width = UnicodeWidthStr::width(line.as_ref());
                            vec![
                                Span::styled(line, parent_style),
                                Span::styled(" ".repeat({ *bubble_width } - 4 - line_width), parent_style),
                            ]
                    })
                    .collect()
            }
            UserMessage::FileHeader(file_name, size, _id) => {
                let file_size: String = format_size(*size, DECIMAL);
                let file_size_len = UnicodeWidthStr::width(file_size.as_str());

                let top_line = "┌──────┬─".to_string() + &"─".repeat(file_size_len) + "─┐ ";
                let mid_line = "│ FILE │".to_string() + &file_size + " │ ";
                let bot_line = "└──────┴─".to_string() + &"─".repeat(file_size_len) + "─┘ ";

                let file_box_style = parent_style.clone().add_modifier(Modifier::BOLD);

                let mut styled_lines: Vec<Vec<Span<'a>>> = vec![
                    vec![Span::styled(top_line, file_box_style)],
                    vec![Span::styled(mid_line, file_box_style)],
                    vec![Span::styled(bot_line, file_box_style)],
                ];

                let name_len = UnicodeWidthStr::width(file_name.as_str());

                styled_lines[0].push(Span::styled(" ".repeat(name_len), parent_style));
                styled_lines[1].push(Span::styled(file_name, parent_style));
                styled_lines[2].push(Span::styled(" ".repeat(name_len), parent_style));

                let total_length = 12 + file_size_len + name_len;

                // Calculate loading bar string based on progress.
                if let Some(loading_bar) = &loading_bar {
                    let locked_loading_bar = &loading_bar.lock().unwrap().loadingbar;

                    match &locked_loading_bar {
                        LoadingBar::Status(loading_bar_status) => {
                            let procentage = (loading_bar_status.position * 100) / loading_bar_status.end;
                            let filled_len = ((total_length - 4) * procentage as usize) / 100;

                            let bar_style = parent_style.clone().fg(Color::Green);

                            styled_lines.push(vec![
                                Span::styled(format!("{:2}% ", procentage), parent_style),
                                Span::styled("═".repeat(filled_len), bar_style),
                                Span::styled("─".repeat(total_length - filled_len), bar_style),
                            ]);
                        },
                        LoadingBar::Error(err) => {
                            let err_style = parent_style.clone().fg(Color::Red);

                            styled_lines.push(vec![
                                Span::styled(format!("ERR: {: <width$}", err, width = total_length - 5), err_style),
                            ]);
                        },
                    }
                }

                let line_width = total_length
                    .min(window_max_width as usize)
                    .max(*bubble_width);

                *bubble_width = line_width as usize;

                styled_lines
            }
        }
    }
}
