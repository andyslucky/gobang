use anyhow::Result;
use async_trait::async_trait;
use tui::{
    backend::Backend,
    layout::{Alignment, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::app::GlobalMessageQueue;
use crate::components::command::CommandInfo;
use crate::config::KeyConfig;
use crate::event::Key;

use super::{Component, DrawableComponent, EventState};

pub struct ErrorComponent {
    pub error: String,
    visible: bool,
    key_config: KeyConfig,
}

impl ErrorComponent {
    pub fn new(key_config: KeyConfig) -> Self {
        Self {
            error: String::new(),
            visible: false,
            key_config,
        }
    }
}

impl ErrorComponent {
    pub fn set(&mut self, error: String) -> anyhow::Result<()> {
        self.error = error;
        self.show()
    }
}

impl DrawableComponent for ErrorComponent {
    fn draw<B: Backend>(&self, f: &mut Frame<B>, _area: Rect, _focused: bool) -> Result<()> {
        if self.visible {
            let width = 65;
            let height = 10;
            let error = Paragraph::new(self.error.to_string())
                .block(Block::default().title("Error").borders(Borders::ALL))
                .style(Style::default().fg(Color::Red))
                .alignment(Alignment::Left)
                .wrap(Wrap { trim: true });
            let area = Rect::new(
                (f.size().width.saturating_sub(width)) / 2,
                (f.size().height.saturating_sub(height)) / 2,
                width.min(f.size().width),
                height.min(f.size().height),
            );
            f.render_widget(Clear, area);
            f.render_widget(error, area);
        }
        Ok(())
    }
}

#[async_trait]
impl Component for ErrorComponent {
    fn commands(&self, _out: &mut Vec<CommandInfo>) {}

    async fn event(
        &mut self,
        key: Key,
        _message_queue: &mut GlobalMessageQueue,
    ) -> Result<EventState> {
        if self.visible {
            if key == self.key_config.exit_popup {
                self.error = String::new();
                self.hide();
                return Ok(EventState::Consumed);
            }
            return Ok(EventState::Consumed);
        }
        Ok(EventState::NotConsumed)
    }

    fn hide(&mut self) {
        self.visible = false;
    }

    fn show(&mut self) -> Result<()> {
        self.visible = true;

        Ok(())
    }
}
