use crate::modules::event_handler;

use super::peer_state::PeerState;
use super::{event_handler::*, peer_list::*};

use ratatui::{
    backend::CrosstermBackend,
    buffer::Buffer,
    crossterm::event::KeyCode,
    layout::{Constraint, Layout, Rect},
    widgets::Widget,
    Terminal,
};

#[derive(PartialEq)]
enum AppPosition {
    PeerList,
    ChatSession,
}

pub struct App {
    peers: PeerList,
    current_screen: AppPosition,
    events: EventHandler,
}

impl App {
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
                    AppPosition::PeerList => match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => {
                            break 'render_loop;
                        }
                        KeyCode::Enter if self.peers.get_selected().is_some() => {
                            self.current_screen = AppPosition::ChatSession;
                        }
                        _ => self.peers.handle_event(&key.code),
                    },
                    AppPosition::ChatSession => match key.code {
                        KeyCode::Esc => {
                            self.current_screen = AppPosition::PeerList;
                        }
                        _ => {
                            if let Some(peer) = self.peers.get_selected() {
                                peer.handle_event(&key.code);
                            } else {
                                self.current_screen = AppPosition::PeerList;
                            }
                        }
                    },
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

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Devide main screen.
        let [mut peers_block, mut msg_block] =
            Layout::horizontal([Constraint::Percentage(30), Constraint::Percentage(70)])
                .areas(area);

        let is_active: bool = self.current_screen == AppPosition::PeerList;

        self.peers.render(&mut peers_block, buf, is_active);

        if let Some(peer) = self.peers.get_selected() {
            // Devide conversation to include editor box.
            let [mut conv_block, mut edit_block] =
                Layout::vertical([Constraint::Percentage(80), Constraint::Percentage(20)])
                    .areas(msg_block);

            let is_active: bool = self.current_screen == AppPosition::ChatSession;

            peer.render_conv(&mut conv_block, buf);
            peer.render_edit(&mut edit_block, buf, is_active);
        } else {
            PeerState::render_empty(&mut msg_block, buf);
        }
    }
}
