extern crate core;

use std::io;

use anyhow::Result;
use crossterm::{
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use log::{debug, error};
use tui::{backend::CrosstermBackend, Terminal};

use crate::app::App;
use crate::event::{Event, Key};

mod app;
mod cli;
mod clipboard;
mod components;
mod config;
mod database;
mod event;
mod saturating_types;
mod sql_utils;
mod ui;
mod version;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    log4rs::init_file("log4rs.yml", Default::default()).unwrap();
    let value = crate::cli::parse();
    let config = config::Config::new(&value.config)?;

    setup_terminal()?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    let events = event::Events::new(250);
    let mut app = App::new(config.clone()).await;

    terminal.clear()?;

    loop {
        terminal.draw(|f| {
            if let Err(err) = app.draw(f) {
                error!("error: {}", err);
                std::process::exit(1);
            }
        })?;
        match events.next()? {
            Event::Input(key) => match app.event(key).await {
                Ok(state) => {
                    // debug!(
                    //     "Key pressed {:?} state consumed {}. Quit key {:?} exit key {:?}",
                    //     key,
                    //     state.is_consumed(),
                    //     app.config.key_config.quit,
                    //     app.config.key_config.exit
                    // );
                    if !state.is_consumed()
                        && (key == app.config.key_config.quit
                            || key == Key::Ctrl(crossterm::event::KeyCode::Char('c'))
                            || key == Key::Ctrl(crossterm::event::KeyCode::Char('C')))
                    {
                        debug!("Exiting main event loop!");
                        break;
                    }
                }
                Err(err) => {
                    error!("error: {}", err);
                    app.error.set(err.to_string())?;
                }
            },
            Event::Tick => (),
        }
    }

    shutdown_terminal();
    terminal.show_cursor()?;

    Ok(())
}

fn setup_terminal() -> Result<()> {
    enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    Ok(())
}

fn shutdown_terminal() {
    let leave_screen = io::stdout().execute(LeaveAlternateScreen).map(|_f| ());

    if let Err(e) = leave_screen {
        eprintln!("leave_screen failed:\n{}", e);
    }

    let leave_raw_mode = disable_raw_mode();

    if let Err(e) = leave_raw_mode {
        eprintln!("leave_raw_mode failed:\n{}", e);
    }
}
