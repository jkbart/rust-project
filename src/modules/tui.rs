use super::{app_state::*, networking::*, protocol::*};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::task::JoinHandle;

use ratatui::{
    backend::{Backend, CrosstermBackend},
    crossterm::{
        event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    Terminal,
};

enum ChatSessionPosition {
    TextEditor(String),
}

enum AppPosition {
    PeerList,
    ChatSession(usize, ChatSessionPosition),
}

struct App {
    peers: Arc<Mutex<Vec<PeerState>>>, // TODO: Hashmap by id.
    current_screen: AppPosition,
    peer_updator: JoinHandle<Result<(), StreamSerializerError>>,
}

impl App {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel::<ConnectionData>();
        let peers: Arc<Mutex<Vec<PeerState>>> = Arc::new(Vec::new().into());
        tokio::task::spawn(search_for_users(tx));

        App {
            peers: peers.clone(),
            current_screen: AppPosition::PeerList,
            peer_updator: tokio::task::spawn(peer_list_updator(peers, rx)),
        }
    }

    pub fn run<W: std::io::Write>(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<W>>,
    ) -> std::io::Result<bool> {
        loop {
            if let Event::Key(key) = event::read()? {
                if key.kind == event::KeyEventKind::Release {
                    // Skip events that are not KeyEventKind::Press
                    continue;
                }
                match &self.current_screen {
                    AppPosition::PeerList => todo!(),
                    AppPosition::ChatSession(idx, position) => {
                        todo!();
                    }
                }
            }
        }

        unimplemented!();
    }
}
