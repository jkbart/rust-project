use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use rand::RngCore;
use crate::modules::event_handler;

use super::{app_state::*, protocol::*, event_handler::*};
use std::ops::Deref;

use ratatui::{
    prelude::*,
    buffer::Buffer,
    backend::{Backend, CrosstermBackend},
    crossterm::event::{KeyCode},
    text::Line,
    layout::{Constraint, Layout, Rect},   
    widgets::{
        List, ListItem, ListState,
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
        let [peers_block, msg_block] = Layout::horizontal([
            Constraint::Min(10),
            Constraint::Min(10),
        ])
        .areas(area);

        {
            let peer_items: Vec<ListItem> = self
                .peers
                .peer_list
                .iter()
                .map(|peer| {
                    ListItem::from(Line::from(vec![
                        (*peer.name).bold(),
                    ]))
                })
                .collect();


            if !peer_items.is_empty() {
                let peer_list = List::new(peer_items)
                    .block(Block::default()
                        .borders(Borders::ALL)
                        .title("Peers:"));
                StatefulWidget::render(peer_list, peers_block, buf, &mut self.peers.state);
            } else {
                let block = Block::default()
                    .title("No users detected!")
                    .borders(ratatui::widgets::Borders::ALL);
                Widget::render(block, peers_block, buf);
            }
        }
        {
            if let Some(peer) = self.peers.get_selected() {
                let msg_items: Vec<ListItem> = peer
                    .conversation
                    .iter()
                    .map(|msg| {
                        match &msg.message.content {
                            MessageContent::Text(txt) =>
                                ListItem::from(Line::from(vec![
                                    (txt.deref()).bold(),
                                ])),
                            MessageContent::Empty() => ListItem::from(Line::from(vec![
                                    "empty msg".bold(),
                                ]))
                        }
                    })
                    .collect();

                let msg_list = List::new(msg_items);
                StatefulWidget::render(msg_list, msg_block, buf, &mut ListState::default());
            }
        }
    }
}
