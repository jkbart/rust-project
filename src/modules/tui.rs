use crate::modules::event_handler;

use super::peer_state::PeerState;
use super::{event_handler::*, peer_list::*};

use ratatui::{
    backend::CrosstermBackend,
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    widgets::Widget,
    Terminal,
};

#[derive(PartialEq)]
pub enum AppPosition {
    PeerList,
    ChatSession,
}

pub struct App<'a> {
    peers: PeerList<'a>,
    current_screen: AppPosition,
    events: EventHandler,
}

impl Default for App<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl App<'_> {
    pub fn new() -> Self {
        App {
            peers: PeerList::new(),
            current_screen: AppPosition::PeerList,
            events: EventHandler::new(),
        }
    }

    pub async fn run<W: std::io::Write>(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<W>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        'render_loop: loop {
            'edit_scope: {
                let Ok(event) = self.events.next().await else {
                    break 'render_loop;
                };
                let event_handler::Event::Key(key) = event else {
                    break 'edit_scope;
                };

                match &mut self.current_screen {
                    AppPosition::PeerList => {
                        if self.peers.handle_event(key, &mut self.current_screen) {
                            break 'render_loop;
                        }
                    }
                    AppPosition::ChatSession => {
                        if let Some(peer) = self.peers.get_selected() {
                            if peer.handle_event(key, &mut self.current_screen) {
                                self.current_screen = AppPosition::PeerList;
                            }
                        } else {
                            self.current_screen = AppPosition::PeerList;
                        }
                    }
                }
            }

            self.peers.update();
            if let Some(peer) = self.peers.get_selected() {
                peer.update();
            }

            terminal.draw(|frame| frame.render_widget(&mut *self, frame.area()))?;
        } // render_loop
        Ok(())
    }
}

impl Widget for &mut App<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Devide main screen.
        let [mut peers_block, mut msg_block] =
            Layout::horizontal([Constraint::Percentage(30), Constraint::Percentage(70)])
                .areas(area);

        let is_active: bool = self.current_screen == AppPosition::PeerList;

        self.peers.render(&mut peers_block, buf, is_active);

        if let Some(peer) = self.peers.get_selected() {
            // Devide conversation to include editor box.

            let is_active: bool = self.current_screen == AppPosition::ChatSession;

            peer.render(&mut msg_block, buf, is_active)
        } else {
            PeerState::render_empty(&mut msg_block, buf);
        }
    }
}
