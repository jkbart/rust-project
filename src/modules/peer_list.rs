use crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::prelude::Stylize;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::Borders;
use ratatui::widgets::List;
use ratatui::widgets::ListItem;
use ratatui::widgets::StatefulWidget;
use ratatui::widgets::Widget;
use ratatui::{
    buffer::Buffer,
    widgets::{Block, ListState},
};

use super::{networking::*, protocol::*};
use cli_log::*;
use std::sync::{Arc, Mutex};
use tokio::task::JoinHandle;

use tokio::sync::mpsc;

use crate::modules::peer_state::*;

pub struct PeerList {
    pub peer_list: Vec<PeerState>,
    pub state: ListState,
    peer_buffer: Arc<Mutex<Vec<PeerState>>>,
    _peer_updator: JoinHandle<Result<(), StreamSerializerError>>,
}

impl PeerList {
    pub fn new() -> Self {
        let peer_buffer: Arc<Mutex<Vec<PeerState>>> = Arc::new(Mutex::new(Vec::new()));

        let (tx_peer_list, rx_peer_list) = mpsc::unbounded_channel::<ConnectionData>();
        tokio::task::spawn(search_for_users(tx_peer_list));

        let peer_updator = tokio::task::spawn(peer_list_updator(peer_buffer.clone(), rx_peer_list));

        PeerList {
            peer_list: Vec::new(),
            state: ListState::default(),
            peer_buffer,
            _peer_updator: peer_updator,
        }
    }

    pub fn update(&mut self) {
        let mut peer_buffer = self.peer_buffer.lock().unwrap();
        self.peer_list.append(&mut peer_buffer);
        if self.state.selected().is_none() && !self.peer_list.is_empty() {
            self.state.select(Some(0));
        }
    }

    pub fn select_next(&mut self) {
        if let Some(idx) = self.state.selected() {
            self.state.select(Some((idx + 1) % self.peer_list.len()));
        }
    }

    pub fn select_previous(&mut self) {
        if let Some(idx) = self.state.selected() {
            self.state.select(Some(
                (idx + self.peer_list.len() - 1) % self.peer_list.len(),
            ));
        }
    }

    pub fn get_selected(&mut self) -> Option<&mut PeerState> {
        self.state.selected().map(|idx| &mut self.peer_list[idx])
    }

    pub fn handle_event(&mut self, keycode: &KeyCode) {
        match &keycode {
            KeyCode::Up => {
                self.select_previous();
            }
            KeyCode::Down => {
                self.select_next();
            }
            _ => {}
        }
    }

    pub fn render(&mut self, block: &mut Rect, buf: &mut Buffer, is_active: bool) {
        let peer_items: Vec<ListItem> = self
            .peer_list
            .iter()
            .enumerate()
            .map(|(idx, peer)| {
                if self.state.selected() != Some(idx) {
                    ListItem::from(Line::from(vec![(*peer.name).bold()]))
                } else {
                    ListItem::from(Line::from(vec![(*peer.name).bold()])).bg(Color::DarkGray)
                }
            })
            .collect();

        if !peer_items.is_empty() {
            let peer_list = List::new(peer_items).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Peers:")
                    .border_style(Style::default().add_modifier(Modifier::BOLD))
                    .border_style(if is_active {
                        Style::default().fg(Color::Green)
                    } else {
                        Style::default()
                    }),
            );
            StatefulWidget::render(peer_list, *block, buf, &mut self.state);
        } else {
            let block2 = Block::default()
                .title("No users detected!")
                .borders(ratatui::widgets::Borders::ALL);
            Widget::render(block2, *block, buf);
        }
    }
}

pub async fn peer_list_updator(
    peers: Arc<Mutex<Vec<PeerState>>>,
    mut peer_queue: mpsc::UnboundedReceiver<ConnectionData>,
) -> Result<(), StreamSerializerError> {
    loop {
        match peer_queue.recv().await {
            Some(connection_data) => {
                info!("New user detected!");
                peers.lock().unwrap().push(connection_data.into());
            }
            None => {
                break Ok(());
            }
        }
    }
}
