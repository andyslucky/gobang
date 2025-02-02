use crate::components::command::CommandInfo;
use crate::config::KeyConfig;
use anyhow::Result;
use async_trait::async_trait;
use tui::{
    backend::Backend,
    layout::{Alignment, Rect},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use super::{Component, DrawableComponent, EventState};

pub struct DebugComponent {
    msg: String,
    visible: bool,
    key_config: KeyConfig,
}

impl DebugComponent {
    #[allow(dead_code)]
    pub fn new(key_config: KeyConfig, msg: String) -> Self {
        Self {
            msg,
            visible: false,
            key_config,
        }
    }
}

impl DrawableComponent for DebugComponent {
    fn draw<B: Backend>(&self, f: &mut Frame<B>, _area: Rect, _focused: bool) -> Result<()> {
        if true {
            let width = 65;
            let height = 10;
            let error = Paragraph::new(self.msg.to_string())
                .block(Block::default().title("Debug").borders(Borders::ALL))
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
impl Component for DebugComponent {
    fn commands(&self, _out: &mut Vec<CommandInfo>) {}

    async fn event(
        &mut self,
        key: crate::event::Key,
        _message_queue: &mut crate::app::GlobalMessageQueue,
    ) -> Result<EventState> {
        if self.visible {
            if key == self.key_config.exit_popup {
                self.msg = String::new();
                self.hide();
                return Ok(EventState::Consumed);
            }
            return Ok(EventState::NotConsumed);
        }
        Ok(EventState::NotConsumed)
    }
}
