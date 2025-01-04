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

pub struct LoadingBar {
    pub position: u64,
    pub end: u64,
    pub changed: bool,
}

pub struct MsgBubble<'a> {
    pub received_from: Option<String>,
    pub message: UserMessage,
    pub loading_bar: Option<Arc<Mutex<LoadingBar>>>, // Used for file downloading.
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
            Style::default().bg(Color::Yellow) // Change background color to Yellow if selected
        } else {
            Style::default()
        };

        let sender = self.received_from.as_deref().unwrap_or("You");

        let name_length = UnicodeWidthStr::width(sender).min(window_max_width as usize - 2);

        let mut bubble_width = (name_length + 2).min(window_max_width as usize);

        let mut middle_lines = bubble_content(
            &self.message,
            &self.loading_bar,
            window_max_width,
            &mut bubble_width,
        ); // Needs to be called before top_name calculations.

        let mut raw_lines = Vec::<String>::new();

        let top_name: String = match self.allignment {
            MsgBubbleAllignment::Left => format!(
                "{:─<width$}",
                &sender[..name_length],
                width = bubble_width - 2
            ),
            MsgBubbleAllignment::Right => format!(
                "{:─>width$}",
                &sender[..name_length],
                width = bubble_width - 2
            ),
        };

        raw_lines.push("┌".to_string() + &top_name + "┐");
        raw_lines.append(&mut middle_lines);
        raw_lines.push("└".to_string() + &"─".repeat(bubble_width - 2) + "┘");

        let rendered_msg: Vec<Line> = raw_lines
            .into_iter()
            .map(|line| {
                Line::from(Span::styled(
                    match self.allignment {
                        MsgBubbleAllignment::Left => {
                            line + &" ".repeat(window_max_width as usize - bubble_width)
                        }
                        MsgBubbleAllignment::Right => {
                            " ".repeat(window_max_width as usize - bubble_width) + &line
                        }
                    },
                    style,
                ))
            })
            .collect();

        let height = rendered_msg.len() as u16;

        self.render_cache = Some(ListCache::new(
            rendered_msg,
            window_max_width,
            height,
            selected,
        ));
    }
}

fn bubble_content(
    msg: &UserMessage,
    loading_bar: &Option<Arc<Mutex<LoadingBar>>>,
    window_max_width: u16,
    bubble_width: &mut usize,
) -> Vec<String> {
    match &msg {
        UserMessage::Text(text) => {
            // Little extra padding, because for text:
            // "C:\Users\jbart\AppData\Local\Packages\Microsoft.WindowsFeedbackHub_8wekyb3d8bbwe\LocalState\{18c6d0aa-02b6-4df0-982c-40fd41c34137}\Capture0.png"
            // textwrap::wrap returned one line longer than was asked. This is a problem with textwrap cargo.
            let mut lines = textwrap::wrap(text, window_max_width as usize - 8);
            if lines.is_empty() {
                lines.push(std::borrow::Cow::Borrowed(" "));
            }

            *bubble_width = (*bubble_width).max(4);
            let max_line_width = lines
                .iter()
                .map(|line| UnicodeWidthStr::width(line.as_ref()) as u16)
                .max()
                .unwrap_or(0)
                .max(*bubble_width as u16 - 4);
            *bubble_width = max_line_width as usize + 4;

            lines
                .into_iter()
                .map(|line| {
                    "│ ".to_string()
                        + &line
                        + &" ".repeat({ *bubble_width } - 4 - UnicodeWidthStr::width(line.as_ref()))
                        + " │"
                })
                .collect()
        }
        UserMessage::FileHeader(name, size, _id) => {
            let size: String = format_size(*size, DECIMAL);

            let mut line = "FILE ".to_string() + &size + " " + name;

            if let Some(loading_bar) = loading_bar {
                let locked_loading_bar = loading_bar.lock().unwrap();
                line = line
                    + " "
                    + format_size(locked_loading_bar.position, DECIMAL).as_str()
                    + "/"
                    + &format_size(locked_loading_bar.end, DECIMAL);
            }

            let line_width = (UnicodeWidthStr::width(line.as_str()))
                .min(window_max_width as usize - 4)
                .max(*bubble_width - 4);
            *bubble_width = line_width as usize + 4;

            vec![
                "│ ".to_string() + format!("{: <width$}", line, width = line_width).as_str() + " │",
            ]
        }
    }
}
