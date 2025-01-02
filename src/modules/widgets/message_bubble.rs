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
        let top_name: String =  match self.allignment {
            MsgBubbleAllignment::Left  => format!("{:─<width$}", &self.sender[..name_length], width = window_max_width as usize - 2),
            MsgBubbleAllignment::Right => format!("{:─>width$}", &self.sender[..name_length], width = window_max_width as usize - 2),
        };

        let top_bar    = "┌".to_string() + &top_name +                                  "┐";         
        let bottom_bar = "└".to_string() + &"─".repeat(window_max_width as usize - 2) + "┘";

        let mut rendered_msg: Vec<Line> = vec![Line::from(Span::styled(top_bar, style))];

        rendered_msg.append(&mut bubble_content(&self.message.content, &style, window_max_width));

        rendered_msg.push(Line::from(Span::styled(bottom_bar, style)));

        let height = rendered_msg.len() as u16;

        self.render_cache = Some(ListCache::new(
            rendered_msg,
            window_max_width,
            height,
            selected,
        ));
    }
}

fn bubble_content(content: & MessageContent, style: &Style, window_max_width: u16) -> Vec<Line<'static>> {
    match &content {
        MessageContent::Text(text) => {
            textwrap::wrap_columns(text, 1, window_max_width as usize, "│", "*unused*", "│")    // *unused* because for only 1 column this argument is not needed.
                .into_iter()
                .map(|line| {
                    Line::from(Span::styled(line, *style))
                })
                .collect()
        },
        MessageContent::Empty() => {
            vec![Line::from(Span::styled("│".to_string() + &"#".repeat(window_max_width as usize - 2) + "│", *style))]
        }
    }
}