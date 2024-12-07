use rand::RngCore;
use crate::modules::event_handler;

use super::peer_state::PeerState;
use super::{peer_list::*, protocol::*, event_handler::*};

use ratatui::{
    buffer::Buffer,
    backend::{Backend, CrosstermBackend},
    crossterm::event::{KeyCode},
    layout::{Constraint, Layout, Rect},   
    widgets::{
        StatefulWidget, Widget,
    },
    Terminal,
};



enum AppPosition {
    PeerList,
    ChatSession, // Idx of peer is in peers state
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
                let Ok(event) = self.events.next().await else { break 'render_loop };
                let event_handler::Event::Key(key) = event else { break 'edit_scope; };

                match &mut self.current_screen {
                    AppPosition::PeerList => {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => {
                                break 'render_loop;
                            },
                            KeyCode::Up => {
                                self.peers.select_previous();
                            },
                            KeyCode::Down => {
                                self.peers.select_next();
                            },
                            KeyCode::Enter if self.peers.get_selected().is_some() => {
                                self.current_screen = AppPosition::ChatSession;
                            },
                            _ => {},
                        }

                    },
                    AppPosition::ChatSession => {
                        match key.code {
                            KeyCode::Char(c) => {
                                // This code repeats 3 times, could be replaced with macro in the future.
                                if let Some(peer) = self.peers.get_selected() {
                                    match &mut peer.next_message.content {
                                        MessageContent::Text(msg) => {
                                            msg.push(c);
                                        }
                                        _ => {}
                                    }
                                } else {
                                    self.current_screen = AppPosition::PeerList;
                                }
                            },
                            KeyCode::Backspace => {
                                if let Some(peer) = self.peers.get_selected() {
                                    match &mut peer.next_message.content {
                                        MessageContent::Text(msg) => {
                                            msg.pop();
                                        }
                                        _ => {}
                                    }
                                } else {
                                    self.current_screen = AppPosition::PeerList;
                                }
                            }
                            KeyCode::Esc => {
                                self.current_screen = AppPosition::PeerList;
                            }
                            KeyCode::Enter => {
                                if let Some(peer) = self.peers.get_selected() {
                                    let mut msg = Message {
                                        content: MessageContent::Text(String::new()),
                                        msg_id: rand::thread_rng().next_u64(),
                                    };
                                    std::mem::swap(&mut msg, &mut peer.next_message);

                                    peer.send(msg);
                                } else {
                                    self.current_screen = AppPosition::PeerList;
                                }
                            }
                            _ => {},
                        }
                    },
                }
            }
            self.peers.update();
            if let Some(peer)  = self.peers.get_selected() {
                peer.update();
            }

            terminal.draw(|frame| frame.render_widget(&mut *self, frame.area()))?;
        } // render_loop
        Ok(())
    }
}


impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let [mut peers_block, mut msg_block] = Layout::horizontal([
            Constraint::Percentage(30),
            Constraint::Percentage(70),
        ])
        .areas(area);

        self.peers.render(&mut peers_block, buf);

        if let Some(peer) = self.peers.get_selected() {
            let [mut conv_block, mut edit_block] = Layout::vertical([
                Constraint::Percentage(80),
                Constraint::Percentage(20),
            ])
            .areas(msg_block);

            peer.render_conv(&mut conv_block, buf);
            peer.render_edit(&mut edit_block, buf);
        } else {
            PeerState::render_empty(&mut msg_block, buf);
        }
    }
}
