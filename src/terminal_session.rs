use anyhow::Result;
use crossterm::ExecutableCommand;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use std::io::stdout;

pub struct Session;

impl Session {
    pub fn enter() -> Result<Self> {
        stdout().execute(EnterAlternateScreen)?;
        if let Err(error) = terminal::enable_raw_mode() {
            let _ = stdout().execute(LeaveAlternateScreen);
            return Err(error.into());
        }
        Ok(Self)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = stdout().execute(LeaveAlternateScreen);
    }
}
