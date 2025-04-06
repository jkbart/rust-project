use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use std::sync::Arc;
use std::sync::Mutex;

use humansize::{format_size, DECIMAL};
use unicode_width::UnicodeWidthStr;

use crate::modules::protocol::*;
use crate::modules::widgets::list_component::*;

#[derive(Debug)]
pub enum MsgBubbleAllignment {
    Left,
    Right,
}

#[derive(Debug)]
pub struct LoadingBarStatus {
    pub position: FileSize,
    pub end: FileSize,
}

// Loading bar is currently used to show progress of file download.
#[derive(Debug)]
pub enum LoadingBar {
    Status(LoadingBarStatus),
    Error(String),
}

#[derive(Debug)]
pub struct LoadingBarWrap {
    pub loadingbar: LoadingBar,
    pub changed: bool, // For cache validation.
}

// If last operation ended and new can begin.
pub fn is_loading_bar_free(ld: &Option<Arc<Mutex<LoadingBarWrap>>>) -> bool {
    match &ld {
        None => true,
        Some(lock) => match &lock.lock().unwrap().loadingbar {
            LoadingBar::Status(LoadingBarStatus { position, end }) => position == end,
            LoadingBar::Error(_) => true,
        },
    }
}

// Struct containing displayable msgs together with cache.
#[derive(Debug)]
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

    // Currently prerender performs a lot of operations on vecs and strings.
    // This is ok since number of msgs is small and also we are caching rendered lines most of the times.
    fn prerender(&mut self, window_max_width: u16, selected: bool) {
        let window_max_width = window_max_width.max(10); // On smaller windows this will cause to mess up visuals but will keep it from panicing.

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
        let name_length = (UnicodeWidthStr::width(sender) as u16).min(window_max_width - 2);

        // Total length of bubble insides (inside "│ " " │"). Will be only increased.
        let mut bubble_inner_width = (name_length.max(2) - 2).min(window_max_width - 4);

        let mut middle_lines: Vec<Vec<Span<'a>>> = Self::formatted_content(
            &self.message,
            &self.loading_bar,
            style,
            window_max_width - 4,
            &mut bubble_inner_width,
        );

        // +/- 2/4 to bubble_width are related to adding "│ " " │"
        let left_padding_len = match self.allignment {
            MsgBubbleAllignment::Left => 0,
            MsgBubbleAllignment::Right => window_max_width - (bubble_inner_width + 4),
        };

        let top_line: Span<'a> = Span::styled(
            match self.allignment {
                MsgBubbleAllignment::Left => format!(
                    "{}┌{:─<width$}┐",
                    " ".repeat(left_padding_len as usize),
                    &sender[..name_length as usize],
                    width = bubble_inner_width as usize + 2
                ),
                MsgBubbleAllignment::Right => format!(
                    "{}┌{:─>width$}┐",
                    " ".repeat(left_padding_len as usize),
                    &sender[..name_length as usize],
                    width = bubble_inner_width as usize + 2
                ),
            },
            style,
        );

        let bot_line: Span<'a> = Span::styled(
            format!(
                "{}└{}┘",
                " ".repeat(left_padding_len as usize),
                "─".repeat(bubble_inner_width as usize + 2)
            ),
            style,
        );

        for mid_line in middle_lines.iter_mut() {
            mid_line.insert(
                0,
                Span::styled(" ".repeat(left_padding_len as usize) + "│ ", style),
            );
            mid_line.push(Span::styled(" │", style));
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
    // Calculate inside of bubble based on content.
    fn formatted_content(
        message: &UserMessage,
        loading_bar: &Option<Arc<Mutex<LoadingBarWrap>>>,
        parent_style: Style,
        window_max_width: u16,
        bubble_inner_width: &mut u16,
    ) -> Vec<Vec<Span<'a>>> {
        match &message {
            UserMessage::Text(text) => {
                // Little extra padding, because for text:
                // "C:\Users\jbart\AppData\Local\Packages\Microsoft.WindowsFeedbackHub_8wekyb3d8bbwe\LocalState\{18c6d0aa-02b6-4df0-982c-40fd41c34137}\Capture0.png"
                // textwrap::wrap returned one line longer than was asked. This is a problem with textwrap cargo.
                let mut lines = textwrap::wrap(text, window_max_width as usize - 5);

                // Ensure that lines are not empty.
                if lines.is_empty() {
                    lines.push(std::borrow::Cow::Borrowed(" "));
                }

                let max_line_width = lines
                    .iter()
                    .map(|line| UnicodeWidthStr::width(line.as_ref()) as u16)
                    .max()
                    .unwrap_or(0)
                    .max(*bubble_inner_width);

                *bubble_inner_width = max_line_width;

                lines
                    .into_iter()
                    .map(|line| {
                        let line_width = UnicodeWidthStr::width(line.as_ref());
                        vec![
                            Span::styled(line.into_owned(), parent_style),
                            Span::styled(
                                " ".repeat((*bubble_inner_width as usize) - line_width),
                                parent_style,
                            ),
                        ]
                    })
                    .collect()
            }
            UserMessage::FileHeader(file_name, size, _id) => {
                let file_size: String = format_size(*size, DECIMAL);
                let file_size_len = UnicodeWidthStr::width(file_size.as_str()) as u16;

                let top_line =
                    "┌──────┬─".to_string() + &"─".repeat(file_size_len as usize) + "─┐ ";
                let mid_line = "│ FILE │ ".to_string() + &file_size + " │ ";
                let bot_line =
                    "└──────┴─".to_string() + &"─".repeat(file_size_len as usize) + "─┘ ";

                let file_box_style = parent_style.add_modifier(Modifier::BOLD);

                let mut styled_lines: Vec<Vec<Span<'a>>> = vec![
                    vec![Span::styled(top_line, file_box_style)],
                    vec![Span::styled(mid_line, file_box_style)],
                    vec![Span::styled(bot_line, file_box_style)],
                ];

                let file_header_len = 12 + file_size_len;

                let name_len = UnicodeWidthStr::width(file_name.as_str()) as u16;

                // We will later adjust it to window_max_width. If window_max_width is small enough, then this bubble can go out of window.
                *bubble_inner_width = (*bubble_inner_width).max(12 + file_size_len + name_len);

                // Calculate loading bar string based on progress.
                if let Some(loading_bar) = &loading_bar {
                    let locked_loading_bar = &loading_bar.lock().unwrap().loadingbar;

                    match &locked_loading_bar {
                        LoadingBar::Status(loading_bar_status) => {
                            let procentage =
                                (loading_bar_status.position * 100) / loading_bar_status.end;
                            let filled_len = ((*bubble_inner_width - 5) * procentage as u16) / 100;

                            let bar_style = parent_style.fg(Color::Green);

                            styled_lines.push(vec![
                                Span::styled(format!("{:3}% ", procentage), parent_style),
                                Span::styled("═".repeat(filled_len as usize), bar_style),
                                Span::styled(
                                    "─".repeat((*bubble_inner_width - 5 - filled_len) as usize),
                                    bar_style,
                                ),
                            ]);
                        }
                        LoadingBar::Error(err) => {
                            let err_style = parent_style.fg(Color::Red);
                            let err_len = UnicodeWidthStr::width(err.as_str()) as u16;

                            *bubble_inner_width =
                                (*bubble_inner_width).max(err_len + 5).min(window_max_width);

                            styled_lines.push(vec![Span::styled(
                                format!(
                                    "ERR: {: <width$}",
                                    err,
                                    width = *bubble_inner_width as usize - 5
                                ),
                                err_style,
                            )]);
                        }
                    }
                }

                styled_lines[0].push(Span::styled(
                    " ".repeat((*bubble_inner_width - file_header_len) as usize),
                    parent_style,
                ));
                styled_lines[1].push(Span::styled(
                    file_name.clone()
                        + &" ".repeat((*bubble_inner_width - file_header_len - name_len) as usize),
                    parent_style,
                ));
                styled_lines[2].push(Span::styled(
                    " ".repeat((*bubble_inner_width - file_header_len) as usize),
                    parent_style,
                ));

                *bubble_inner_width = (*bubble_inner_width).min(window_max_width);

                styled_lines
            }
        }
    }
}
