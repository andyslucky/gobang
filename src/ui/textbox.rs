use anyhow::Result;
use async_trait::async_trait;
use tui::backend::Backend;
use tui::Frame;
use tui::layout::Rect;
use tui::style::{Color, Style};
use tui::text::Spans;
use tui::widgets::{Block, Borders, Paragraph};
use unicode_width::UnicodeWidthStr;

use crate::components::*;
use crate::Key;

pub struct TextBox {
    placeholder: Option<String>,
    input: Vec<char>,
    input_idx: usize,
    input_cursor_position: u16,
}

impl TextBox {
    pub fn new(placeholder: Option<String>) -> Self {
        Self {
            placeholder,
            input: Vec::new(),
            input_idx: 0,
            input_cursor_position: 0,
        }
    }

    pub fn input_str(&self) -> String {
        self.input.iter().collect()
    }

    pub fn reset(&mut self) {
        self.input = Vec::new();
        self.input_idx = 0;
        self.input_cursor_position = 0;
    }
}

impl DrawableComponent for TextBox {
    fn draw<B: Backend>(&self, f: &mut Frame<B>, area: Rect, focused: bool) -> Result<()> {
        let query = Paragraph::new(Spans::from(format!(
            "{:w$}",
            if self.input.is_empty() && !focused {
                if let Some(placeholder) = &self.placeholder { placeholder.clone() } else { "".to_string() }
            } else {
                self.input_str()
            },
            w = area.width as usize
        ))).style(if focused {
            Style::default()
        } else {
            Style::default().fg(Color::DarkGray)
        })
            .block(Block::default().borders(Borders::BOTTOM));
        f.render_widget(query, area);

        if focused {
            f.set_cursor(
                (area.x + self.input_cursor_position).min(area.right().saturating_sub(1)),
                area.y,
            )
        }
        Ok(())
    }
}

#[async_trait]
impl Component for TextBox {
    fn commands(&self, _out: &mut Vec<CommandInfo>) {}

    async fn event(&mut self, key: crate::event::Key, _message_queue: &mut crate::app::GlobalMessageQueue) -> Result<EventState> {
        match key {
            Key::Char(c) => {
                self.input.insert(self.input_idx, c);
                self.input_idx += 1;
                self.input_cursor_position += compute_character_width(c);

                return Ok(EventState::Consumed);
            }
            Key::Backspace => {
                if !self.input.is_empty() && self.input_idx > 0 {
                    let last_c = self.input.remove(self.input_idx - 1);
                    self.input_idx -= 1;
                    self.input_cursor_position -= compute_character_width(last_c);
                }
                return Ok(EventState::Consumed);
            }
            Key::Left => {
                if !self.input.is_empty() && self.input_idx > 0 {
                    self.input_idx -= 1;
                    self.input_cursor_position = self
                        .input_cursor_position
                        .saturating_sub(compute_character_width(self.input[self.input_idx]));
                }
                return Ok(EventState::Consumed);
            }
            Key::Ctrl('a') => {
                if !self.input.is_empty() && self.input_idx > 0 {
                    self.input_idx = 0;
                    self.input_cursor_position = 0
                }
                return Ok(EventState::Consumed);
            }
            Key::Right => {
                if self.input_idx < self.input.len() {
                    let next_c = self.input[self.input_idx];
                    self.input_idx += 1;
                    self.input_cursor_position += compute_character_width(next_c);
                }
                return Ok(EventState::Consumed);
            }
            Key::Ctrl('e') => {
                if self.input_idx < self.input.len() {
                    self.input_idx = self.input.len();
                    self.input_cursor_position = self.input_str().width() as u16;
                }
                return Ok(EventState::Consumed);
            }
            _ => (),
        }

        Ok(EventState::NotConsumed)
    }
}
