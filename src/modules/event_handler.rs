// source: https://ratatui.rs/tutorials/counter-async-app/async-event-stream/
use futures::{FutureExt, StreamExt};
use ratatui::crossterm::event::KeyEvent;
use tokio::{sync::mpsc, task::JoinHandle};

#[derive(Clone, Copy, Debug)]
pub enum Event {
    Error,
    Tick,
    Key(KeyEvent),
}

#[derive(Debug)]
pub struct EventHandler {
    _tx: mpsc::UnboundedSender<Event>,
    rx: mpsc::UnboundedReceiver<Event>,
    _task: Option<JoinHandle<()>>,
}

impl Default for EventHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl EventHandler {
    pub fn new() -> Self {
        let tick_rate = std::time::Duration::from_millis(250);

        let (tx, rx) = mpsc::unbounded_channel();
        let _tx = tx.clone();

        let task = tokio::spawn(async move {
            let mut reader = crossterm::event::EventStream::new();
            let mut interval = tokio::time::interval(tick_rate);
            loop {
                let delay = interval.tick();
                let crossterm_event = reader.next().fuse();
                tokio::select! {
                  maybe_event = crossterm_event => {
                    match maybe_event {
                      Some(Ok(crossterm::event::Event::Key(key))) => {
                        tx.send(Event::Key(key)).unwrap();
                      }
                      Some(Err(_)) => {
                        tx.send(Event::Error).unwrap();
                      }
                      _ => {},
                    }
                  },
                  _ = delay => {
                      tx.send(Event::Tick).unwrap();
                  },
                }
            }
        });

        Self {
            _tx,
            rx,
            _task: Some(task),
        }
    }

    pub async fn next(&mut self) -> Result<Event, &str> {
        self.rx.recv().await.ok_or("Failed to get event")
    }
}
