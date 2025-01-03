use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::text::{Line, Span};

use unicode_width::UnicodeWidthStr;
use humansize::{format_size, DECIMAL};

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

pub struct MsgBubble {
    sender: String,
    message: UserMessage,
    loading_bar: Option<LoadingBar>,    // Used for file downloading.
    allignment: MsgBubbleAllignment,
    render_cache: Option<ListCache>,
}


impl MsgBubble {
    pub fn new(
        sender: String,
        message: UserMessage,
        allignment: MsgBubbleAllignment
    ) -> Self {
        MsgBubble {
            sender,
            message,
            loading_bar: None,
            allignment,
            render_cache: None,
        }
    }
}

impl ListItem for MsgBubble {
    fn get_cache(&mut self) -> &mut Option<ListCache> {
        if self.loading_bar.as_ref().is_some_and(|lb| lb.changed) {
            self.render_cache = None;
        }

        &mut self.render_cache
    }

    fn prerender(&mut self, window_max_width: u16, selected: bool) {
        if let Some(loading_bar) = self.loading_bar.as_mut() {
            loading_bar.changed = false;
        }

        let style = if selected {
            Style::default().bg(Color::Yellow) // Change background color to Yellow if selected
        } else {
            Style::default()
        };

        let name_length = UnicodeWidthStr::width(self.sender.as_str()).min(window_max_width as usize - 2);

        let mut bubble_width = (name_length + 2).min(window_max_width as usize);

        let mut middle_lines = bubble_content(&self.message, &self.loading_bar, window_max_width, &mut bubble_width); // Needs to be called before top_name calculations.

        let mut raw_lines = Vec::<String>::new();

        let top_name: String =  match self.allignment {
            MsgBubbleAllignment::Left  => format!("{:─<width$}", &self.sender[..name_length], width = bubble_width as usize - 2),
            MsgBubbleAllignment::Right => format!("{:─>width$}", &self.sender[..name_length], width = bubble_width as usize - 2),
        };

        raw_lines.push("┌".to_string() + &top_name +                              "┐");
        raw_lines.append(&mut middle_lines);
        raw_lines.push("└".to_string() + &"─".repeat(bubble_width as usize - 2) + "┘");

        let rendered_msg: Vec<Line> = raw_lines.into_iter().map(|line| {
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
        }).collect();

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
    loading_bar: &Option<LoadingBar>,
    window_max_width: u16,
    bubble_width: &mut usize
) -> Vec<String> {
    match &msg {
        UserMessage::Text(text) => {
            let mut lines = textwrap::wrap(text, window_max_width as usize - 4);
            if lines.is_empty() {
                lines.push(std::borrow::Cow::Borrowed(" "));
            }

            let max_line_width = lines.iter().map(|line| UnicodeWidthStr::width(line.as_ref()) as u16).max().unwrap().max(*bubble_width as u16 - 4);
            *bubble_width = max_line_width as usize + 4;

            lines
                .into_iter()
                .map(|line| {
                        "│ ".to_string() + &line + &" ".repeat(*bubble_width as usize - 4 - UnicodeWidthStr::width(line.as_ref())) + " │"
                })
                .collect()
        },
        UserMessage::FileHeader(name, size, _id) => {
            let size: String = format_size(*size, DECIMAL);

            let mut line = "FILE ".to_string() + &size +  " " + name;

            if let Some(loading_bar) = loading_bar {
                line = line + " " + format_size(loading_bar.position, DECIMAL).as_str() + "/" + &format_size(loading_bar.end, DECIMAL);
            }

            let line_width = (UnicodeWidthStr::width(line.as_str())).min(window_max_width as usize - 4).max(*bubble_width - 4);
        
            vec!["│ ".to_string() + format!("{: <width$}", line, width = line_width).as_str() + " │"]
        }
    }
}