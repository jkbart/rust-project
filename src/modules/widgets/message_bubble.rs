use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use crate::modules::protocol::{Message, MessageContent};
use crate::modules::widgets::list_component::*;

pub enum MsgBubbleAllignment{
    Left,
    Right,
}

pub struct MsgBubble {
    sender: String,
    message: Message,
    allignment: MsgBubbleAllignment,
    render_cache: Option<ListCache>,
}


impl MsgBubble {
    pub fn new(
        sender: String,
        message: Message,
        allignment: MsgBubbleAllignment
    ) -> Self {
        MsgBubble {
            sender,
            message,
            allignment,
            render_cache: None,
        }
    }
}

impl ListItem for MsgBubble {
    fn get_cache(&mut self) -> &mut Option<ListCache> {
        &mut self.render_cache
    }

    fn prerender(&mut self, window_max_width: u16, selected: bool) {
        let style = if selected {
            Style::default().bg(Color::Yellow) // Change background color to Yellow if selected
        } else {
            Style::default()
        };

        let name_length = UnicodeWidthStr::width(self.sender.as_str()).min(window_max_width as usize - 2);

        let mut bubble_width = (name_length + 2).min(window_max_width as usize);

        let mut middle_lines = bubble_content(&self.message.content, window_max_width, &mut bubble_width); // Needs to be called before top_name calculations.

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
    content: & MessageContent,
    window_max_width: u16,
    bubble_width: &mut usize
) -> Vec<String> {
    match &content {
        MessageContent::Text(text) => {
            let mut lines = textwrap::wrap(text, window_max_width as usize - 2);
            if lines.is_empty() {
                lines.push(std::borrow::Cow::Borrowed(" "));
            }

            let max_line_width = lines.iter().map(|line| UnicodeWidthStr::width(line.as_ref()) as u16).max().unwrap().max(*bubble_width as u16 - 2);
            *bubble_width = max_line_width as usize + 2;

            lines
                .into_iter()
                .map(|line| {
                        "│".to_string() + &line + &" ".repeat(*bubble_width as usize - 2 - UnicodeWidthStr::width(line.as_ref())) + "│"
                })
                .collect()
        },
        MessageContent::Empty() => {
            vec!["│".to_string() + &"#".repeat(window_max_width as usize - 2) + "│"]
        }
    }
}