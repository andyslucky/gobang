use anyhow::Result;
use async_trait::async_trait;
use log::debug;
use tui::backend::Backend;
use tui::Frame;
use tui::layout::Rect;
use tui::style::{Color, Style};
use tui::text::Spans;
use tui::widgets::{Block, Borders, Paragraph};
use unicode_width::UnicodeWidthStr;

use crate::components::*;
use crate::components::EventState::Consumed;
use crate::Key;

#[derive(Debug)]
pub struct TextBox {
    placeholder: Option<String>,
    input: Vec<char>,
    input_cursor_position: usize,
}

impl TextBox {
    pub fn new(placeholder: Option<String>) -> Self {
        Self {
            placeholder,
            input: Vec::new(),
            input_cursor_position: 0,
        }
    }

    pub fn input_str(&self) -> String {
        self.input.iter().collect()
    }

    pub fn reset(&mut self) {
        self.input = Vec::new();
        self.input_cursor_position = 0;
    }
}

impl DrawableComponent for TextBox {
    fn draw<B: Backend>(&self, f: &mut Frame<B>, area: Rect, focused: bool) -> Result<()> {
        let curs_offset : usize = (0..self.input_cursor_position).map( |index| compute_character_width(&self.input[index as usize]) as usize).sum();
        let text = Paragraph::new(Spans::from(format!(
            "{:w$}",
            if self.input.is_empty() {
                if let Some(placeholder) = &self.placeholder { placeholder.clone() } else { "".to_string() }
            } else {
                self.input_str()
            },
            w = area.width as usize
        ))).style(if focused && !self.input.is_empty() {
            Style::default()
        } else {
            Style::default().fg(Color::DarkGray)
        }).block(Block::default().borders(Borders::BOTTOM));
        f.render_widget(text, area);

        if focused {
            f.set_cursor(
                (area.x + curs_offset as u16).min(area.right().saturating_sub(1)),
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
        debug!("Text box state {:?}", self);
        match key {
            Key::Char(c) => {
                self.input.insert(self.input_cursor_position, c);
                self.input_cursor_position += 1;

                return Ok(EventState::Consumed);
            }
            Key::Delete => {
                if !self.input.is_empty() &&
                    self.input_cursor_position as usize <= self.input.len().saturating_sub(1) {
                    self.input.remove(self.input_cursor_position);
                }
                return Ok(Consumed);
            }
            Key::Backspace => {
                if !self.input.is_empty() && self.input_cursor_position > 0 {
                    self.input_cursor_position -= 1;
                    self.input.remove(self.input_cursor_position);
                }
                return Ok(EventState::Consumed);
            }
            Key::Left => {
                if !self.input.is_empty() && self.input_cursor_position > 0 {
                    self.input_cursor_position = self
                        .input_cursor_position
                        .saturating_sub(1);
                }
                return Ok(EventState::Consumed);
            }
            Key::Right => {
                if self.input_cursor_position < self.input.len() {
                    self.input_cursor_position += 1;
                }
                return Ok(EventState::Consumed);
            }
            Key::Ctrl('a') | Key::Home => {
                self.input_cursor_position = 0;
                return Ok(EventState::Consumed);
            }
            Key::Ctrl('e') | Key::End => {
                self.input_cursor_position = self.input.len();
                return Ok(EventState::Consumed);
            }
            _ => (),
        }

        Ok(EventState::NotConsumed)
    }
}
