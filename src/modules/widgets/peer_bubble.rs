use std::sync::Arc;
use ratatui::style::Style;
use ratatui::style::Color;
use ratatui::text::Span;
use ratatui::text::Line;
use std::net::SocketAddr;
use unicode_width::UnicodeWidthStr;

use crate::modules::widgets::list_component::*;

struct PeerBubble {
    name: Arc<String>,
    addr: String,
    render_cache: Option<ListCache>,
}

impl PeerBubble {
    fn new(name: Arc<String>, addr: SocketAddr) -> Self {
        PeerBubble { 
            name,
            addr: addr.to_string(),
            render_cache: None,
        }
    }
}

impl ListComponent for PeerBubble {
    fn get_cache(&mut self) -> &mut Option<ListCache> {
        &mut self.render_cache
    }

    fn prerender(&mut self, window_max_width: u16, selected: bool) {
        let bottom_address_length = UnicodeWidthStr::width(self.addr.as_str()).min(window_max_width as usize - 2);
        let middle_name_length    = UnicodeWidthStr::width(self.name.as_str()).min(window_max_width as usize - 2);
        let bottom_address: String = format!("{:─<width$}", &self.addr[..bottom_address_length], width = window_max_width as usize - 2);
        let middle_name:    String = format!("{:─<width$}", &self.name[..middle_name_length], width = window_max_width as usize - 2);

        let style = if selected {
            Style::default().bg(Color::Yellow) // Change background color to Yellow if selected
        } else {
            Style::default()
        };

        let top_bar    = "┌".to_string() + &"─".repeat(window_max_width as usize - 2) + "┐";
        let middle_bar = "|".to_string() + &middle_name                               + "|";
        let bottom_bar = "└".to_string() + &bottom_address                            + "┘";

        let top_bar = Span::styled(top_bar, style);
        let middle_bar = Span::styled(middle_bar, style);
        let bottom_bar = Span::styled(bottom_bar, style);

        let block_lines: Vec<Line> = vec![
            Line::from(vec![top_bar]),
            Line::from(vec![middle_bar]),
            Line::from(vec![bottom_bar]),
        ];

        self.render_cache = Some(ListCache::new(
            block_lines,
            window_max_width,
            3,
            selected,
        ));
    }
}