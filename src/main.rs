use cli_log::*;
use std::error::Error;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::event::EnableMouseCapture;
use crossterm::terminal::LeaveAlternateScreen;
use crossterm::event::DisableMouseCapture;
use std::io;
use crossterm::execute;
use crossterm::terminal::enable_raw_mode;
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;
use crossterm::terminal::disable_raw_mode;
// use env_logger;
// use log::info;
// use log::LevelFilter;
// use rust_project::config::USER_NAME;
// use rust_project::modules::networking::*;
// use tokio::sync::mpsc;
use rust_project::modules::tui;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {

    init_cli_log!();

    error!("log test");
    // env_logger::Builder::new()
    //     .filter(None, LevelFilter::Trace) // Log all levels
    //     .target(env_logger::Target::Stderr) // Direct logs to stderr
    //     .init();

    // info!("My name: {}", *USER_NAME);

    // let (rx, mut tx) = mpsc::unbounded_channel::<ConnectionData>();

    // let rx_clone = rx.clone();

    // tokio::task::spawn(async move {
    //     match search_for_users(rx_clone).await {
    //         Ok(()) => {}
    //         Err(e) => {
    //             info!("search_for_users ended with error {:?}!", e);
    //         }
    //     }
    // });

    // while let Some(conn) = tx.recv().await {
    //     println!("New conn: {}", conn.peer_name);
    // }
    enable_raw_mode()?;
    let mut stderr = io::stderr(); // This is a special case. Normally using stdout is fine
    execute!(stderr, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stderr);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let _ = tui::App::new().run(&mut terminal).await;

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    ).unwrap();
    terminal.show_cursor()?;

    Ok(())
}
