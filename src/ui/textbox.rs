use anyhow::Result;
use async_trait::async_trait;
use log::debug;
use tui::backend::Backend;
use tui::Frame;
use tui::layout::{Constraint, Layout, Rect};
use tui::style::{Color, Style};
use tui::text::Spans;
use tui::widgets::{Block, Borders, Paragraph};
use unicode_width::UnicodeWidthStr;

use crate::components::*;
use crate::components::EventState::Consumed;
use crate::Key;
use crate::ui::ComponentStyles;


#[derive(Debug)]
pub struct TextBox {
    placeholder: Option<String>,
    component_styles : Option<ComponentStyles>,
    input: Vec<char>,
    input_cursor_position: usize,
}

impl Default for TextBox {
    fn default() -> Self {
        Self {
            placeholder: None,
            component_styles: None,
            input: Vec::new(),
            input_cursor_position: 0,
        }
    }
}

impl TextBox {
    pub fn with_placeholder<S : Into<String>>(mut self, placeholder : S) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    pub fn with_styles(mut self, styles : ComponentStyles) -> Self {
        self.component_styles = Some(styles);
        self
    }

    pub fn input_str(&self) -> String {
        self.input.iter().collect()
    }

    pub fn set_str(&mut self, value : &String) {
        self.input = value.chars().collect();
        self.input_cursor_position = self.input.len();
    }

    pub fn reset(&mut self) {
        self.input = Vec::new();
        self.input_cursor_position = 0;
    }
}

impl DrawableComponent for TextBox {
    fn draw<B: Backend>(&self, f: &mut Frame<B>, area: Rect, focused: bool) -> Result<()> {
        // debug!("Drawing textbox {:?} \nwith area {:?}", self, area);
        let curs_x_offset: usize = (0..self.input_cursor_position).map( |index| compute_character_width(&self.input[index as usize]) as usize).sum();

        // TODO: Implement text-align
        let borders =
            if let Some(styles) = &self.component_styles {
                styles.borders.unwrap_or(Borders::ALL)
            } else {
                Borders::ALL
            };

        let text = Paragraph::new(Spans::from(format!(
            "{:w$}",
            if self.input.is_empty() {
                if let Some(placeholder) = &self.placeholder { placeholder.clone() } else { "".to_string() }
            } else {
                self.input_str()
            },
            w = area.width as usize
        ))).style(if focused && !self.input.is_empty()  {
            Style::default()
        } else {
            Style::default().fg(Color::DarkGray)
        }).block(Block::default().borders(Borders::ALL));
        f.render_widget(text, area);

        let cursor_y_pos = area.y + (area.height / 2);
        if focused {
            f.set_cursor(
                (area.x + curs_x_offset as u16) + 1,
                cursor_y_pos,
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
