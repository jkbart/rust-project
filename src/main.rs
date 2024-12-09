use cli_log::*;
use crossterm::event::DisableMouseCapture;
use crossterm::event::EnableMouseCapture;
use crossterm::execute;
use crossterm::terminal::disable_raw_mode;
use crossterm::terminal::enable_raw_mode;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::terminal::LeaveAlternateScreen;
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;
use rust_project::modules::tui;
use std::error::Error;
use std::io;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    init_cli_log!();

    // Prepare terminal for rendering.
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let _ = tui::App::new().run(&mut terminal).await;

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .unwrap();
    terminal.show_cursor()?;

    Ok(())
}
