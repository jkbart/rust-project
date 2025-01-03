use crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::widgets::Borders;
use ratatui::widgets::Widget;
use ratatui::{
    buffer::Buffer,
    widgets::Block,
};

use super::{networking::*, protocol::*};
use cli_log::*;
use std::sync::{Arc, Mutex};
use tokio::task::JoinHandle;

use tokio::sync::mpsc;

use crate::modules::peer_state::*;

use crate::modules::widgets::list_component::*;

pub struct PeerList {
    pub peer_list: ListComponent<PeerState>,
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
            peer_list: ListComponent::new(ListBegin::Top, ListTop::First),
            peer_buffer,
            _peer_updator: peer_updator,
        }
    }

    pub fn update(&mut self) {
        let mut peer_buffer = self.peer_buffer.lock().unwrap();

        self.peer_list.append(&mut peer_buffer);

        if self.peer_list.get_selected_idx().is_none() && !self.peer_list.is_empty() {
            self.peer_list.select(0);
        }
    }

    pub fn handle_event(&mut self, keycode: &KeyCode) {
        match &keycode {
            KeyCode::Up => {
                self.peer_list.go_up();
            }
            KeyCode::Down => {
                self.peer_list.go_down();
            }
            _ => {}
        }
    }

    pub fn get_selected(&mut self) -> Option<&mut PeerState> {
        self.peer_list.get_selected()
    }

    pub fn render(&mut self, rect: &mut Rect, buf: &mut Buffer, is_active: bool) {
        if self.peer_list.is_empty() {
            let block = Block::default()
                .title("No users detected!")
                .borders(ratatui::widgets::Borders::ALL);
            Widget::render(block, *rect, buf);
        } else {
            let block = Block::default()
                .borders(Borders::ALL)
                .title("Peers:")
                .border_style(Style::default().add_modifier(Modifier::BOLD))
                .border_style(if is_active {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default()
                });

            let content_area = block.inner(*rect);
            
            Widget::render(block, *rect, buf);

            self.peer_list.render(content_area, buf);
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
