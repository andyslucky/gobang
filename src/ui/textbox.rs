use anyhow::Result;
use async_trait::async_trait;
use itertools::Itertools;
use log::{debug, error};
use tui::backend::Backend;
use tui::layout::{Constraint, Direction, Layout, Margin, Rect};
use tui::style::{Color, Style};
use tui::text::Spans;
use tui::widgets::{Block, Borders, Paragraph};
use tui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::components::EventState::{Consumed, NotConsumed};
use crate::components::*;
use crate::ui::ComponentStyles;
use crate::{sql_utils, Key};

#[derive(Debug)]
pub struct TextBox {
    placeholder: Option<String>,
    component_styles: Option<ComponentStyles>,
    label: Option<String>,
    input: Vec<char>,
    input_cursor_position: usize,
}

impl Default for TextBox {
    fn default() -> Self {
        Self {
            placeholder: None,
            component_styles: None,
            label: None,
            input: Vec::new(),
            input_cursor_position: 0,
        }
    }
}

impl TextBox {
    pub fn with_placeholder<S: Into<String>>(mut self, placeholder: S) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    pub fn with_styles(mut self, styles: ComponentStyles) -> Self {
        self.component_styles = Some(styles);
        self
    }

    pub fn with_label<S: Into<String>>(mut self, label: S) -> Self {
        self.set_label(label);
        self
    }

    pub fn input_str(&self) -> String {
        self.input.iter().collect()
    }

    /// Returns the text in the input buffer after the last separator (punctuation, operators, etc.)
    pub fn last_word_part(&self) -> Option<String> {
        let input_str: String = self.input[..self.input_cursor_position].iter().collect();
        if let Some(pat_ind) = sql_utils::find_last_separator(&input_str) {
            let last_word_part: String = self.input
                [(pat_ind.index + pat_ind.length)..self.input_cursor_position]
                .iter()
                .collect();
            return Some(last_word_part);
        }
        return Some(input_str);
    }

    pub fn set_str(&mut self, value: &String) {
        self.input = value.chars().collect();
        self.input_cursor_position = self.input.len();
    }

    pub fn set_label<S: Into<String>>(&mut self, label: S) {
        self.label = Some(format!("{} ", label.into()));
    }

    pub fn reset(&mut self) {
        self.input = Vec::new();
        self.input_cursor_position = 0;
    }

    pub fn cursor_position(&self, area: &Rect) -> (u16, u16) {
        let label_length: usize = if let Some(label) = &self.label {
            label
                .chars()
                .map(|c| compute_character_width(&c) as usize)
                .sum()
        } else {
            0
        };

        let curs_x_offset: usize = (0..self.input_cursor_position)
            .map(|index| compute_character_width(&self.input[index as usize]) as usize)
            .sum::<usize>()
            + label_length;
        let cursor_y_pos = area.y + (area.height / 2);

        return ((area.x + curs_x_offset as u16) + 1, cursor_y_pos);
    }

    pub fn replace_last_word_part<S: Into<String>>(&mut self, text: S) {
        let input_str: String = self.input[..self.input_cursor_position].iter().collect();
        if let Some(pat_ind) = sql_utils::find_last_separator(&input_str) {
            let text = text.into();
            let prefix = &self.input[0..pat_ind.index + pat_ind.length];
            self.input = prefix.iter().map(|c| *c).chain(text.chars()).collect();
        } else {
            self.input = text.into().chars().collect();
        }
        self.input_cursor_position = self.input.len();
    }
}

impl DrawableComponent for TextBox {
    fn draw<B: Backend>(&self, f: &mut Frame<B>, area: Rect, focused: bool) -> Result<()> {
        // debug!("Drawing textbox {:?} \nwith area {:?}", self, area);
        let label_length: usize = if let Some(label) = &self.label {
            label
                .chars()
                .map(|c| compute_character_width(&c) as usize)
                .sum()
        } else {
            0
        };

        // TODO: Implement text-align
        let text_field_block = Block::default().borders(Borders::ALL).style(if focused {
            Style::default()
        } else {
            Style::default().fg(Color::DarkGray)
        });
        f.render_widget(text_field_block, area);

        let mut text_rect = area.inner(&Margin {
            vertical: 1,
            horizontal: 1,
        });

        if let Some(label) = &self.label {
            let label = Paragraph::new(label.as_str())
                .style(Style::default().fg(Color::Rgb(0xea, 0x59, 0x0b)));
            let areas = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(vec![
                    Constraint::Length(label_length as u16),
                    Constraint::Length(area.width - label_length as u16),
                ])
                .split(text_rect);
            let label_rect = areas[0];
            text_rect = areas[1];
            f.render_widget(label, label_rect);
        }

        let text = Paragraph::new(Spans::from(format!(
            "{:w$}",
            if self.input.is_empty() {
                if let Some(placeholder) = &self.placeholder {
                    placeholder.clone()
                } else {
                    "".to_string()
                }
            } else {
                self.input_str()
            },
            w = text_rect.width as usize
        )))
        .style(if focused && !self.input.is_empty() {
            Style::default()
        } else {
            Style::default().fg(Color::DarkGray)
        });
        f.render_widget(text, text_rect);

        let cursor_pos = self.cursor_position(&area);
        if focused {
            f.set_cursor(cursor_pos.0, cursor_pos.1)
        }
        Ok(())
    }
}

#[async_trait]
impl Component for TextBox {
    fn commands(&self, _out: &mut Vec<CommandInfo>) {}

    async fn event(
        &mut self,
        key: crate::event::Key,
        _message_queue: &mut crate::app::GlobalMessageQueue,
    ) -> Result<EventState> {
        return match key {
            Key::Char(c) => {
                self.input.insert(self.input_cursor_position, c);
                self.input_cursor_position += 1;

                Ok(EventState::Consumed)
            }
            Key::Delete => {
                if !self.input.is_empty()
                    && self.input_cursor_position as usize <= self.input.len().saturating_sub(1)
                {
                    self.input.remove(self.input_cursor_position);
                }
                Ok(Consumed)
            }

            Key::Ctrl('\u{08}') => {
                let input_str: String = self.input.clone().into_iter().collect();
                if let Some(pos) = sql_utils::find_last_separator(&input_str) {
                    self.input = self.input[0..pos.index].into();
                    self.input_cursor_position = pos.index;
                } else {
                    self.input.clear();
                    self.input_cursor_position = 0;
                }
                Ok(Consumed)
            }

            Key::Backspace => {
                if !self.input.is_empty() && self.input_cursor_position > 0 {
                    self.input_cursor_position -= 1;
                    self.input.remove(self.input_cursor_position);
                }
                Ok(EventState::Consumed)
            }
            Key::Left => {
                if !self.input.is_empty() && self.input_cursor_position > 0 {
                    self.input_cursor_position = self.input_cursor_position.saturating_sub(1);
                }
                Ok(EventState::Consumed)
            }
            Key::Right => {
                if self.input_cursor_position < self.input.len() {
                    self.input_cursor_position += 1;
                }
                Ok(EventState::Consumed)
            }
            Key::Ctrl('a') | Key::Home => {
                self.input_cursor_position = 0;
                Ok(EventState::Consumed)
            }
            Key::Ctrl('e') | Key::End => {
                self.input_cursor_position = self.input.len();
                Ok(EventState::Consumed)
            }

            _ => Ok(NotConsumed),
        };
    }
}
