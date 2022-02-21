use anyhow::Result;
use async_trait::async_trait;
use crossterm::event;
use crossterm::event::KeyCode;
use tui::backend::Backend;
use tui::layout::{Constraint, Direction, Layout, Margin, Rect};
use tui::style::{Color, Style};
use tui::text::Spans;
use tui::widgets::{Block, Borders, Paragraph};
use tui::Frame;

use crate::components::completion::FilterableCompletionSource;
use crate::components::EventState::{Consumed, NotConsumed};
use crate::components::*;
use crate::config::KeyConfig;
use crate::ui::ComponentStyles;
use crate::{sql_utils, Key};

// #[derive(Debug)]
pub struct TextBox {
    placeholder: Option<String>,
    component_styles: Option<ComponentStyles>,
    label: Option<String>,
    input: Vec<char>,
    input_cursor_position: usize,
    completion: Option<CompletionComponent>,
}

impl Default for TextBox {
    fn default() -> Self {
        Self {
            placeholder: None,
            component_styles: None,
            label: None,
            input: Vec::new(),
            input_cursor_position: 0,
            completion: None,
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

    pub fn with_completion(mut self, key_config: KeyConfig) -> Self {
        let c = CompletionComponent::new(key_config);
        self.completion = Some(c);
        self
    }

    /// Updates the embeded completion element's completion source.
    /// If no completion element is present, this fn has no effect.
    pub fn update_completion_src(&mut self, src: Box<dyn FilterableCompletionSource>) {
        if let Some(c) = &mut self.completion {
            c.completion_source = src;
        }
    }

    /// Collects the input buffer into a String
    pub fn get_text(&self) -> String {
        self.input.iter().collect()
    }

    /// Returns the text in the input buffer between the last separator (punctuation, operators, etc.) and the cursor
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

    /// Sets the value in the text-box's buffer
    pub fn set_str(&mut self, value: &String) {
        self.input = value.chars().collect();
        self.input_cursor_position = self.input.len();
    }

    /// Sets the text-box's label text
    pub fn set_label<S: Into<String>>(&mut self, label: S) {
        self.label = Some(format!("{} ", label.into()));
    }

    /// Resets the text buffer and the embeded completion element (if present)
    pub fn reset(&mut self) {
        self.input = Vec::new();
        self.input_cursor_position = 0;
        if let Some(c) = &mut self.completion {
            c.reset();
        }
    }

    fn cursor_position(&self, area: &Rect) -> (u16, u16) {
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

    /// Replaces the text between the last separator and the cursor with the arg `text`
    ///
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

    /// Attempts to complete the last word/word part before the cursor
    /// returns true if the last word part, relative to the cursor, was replaced else false
    async fn complete_word(&mut self) -> bool {
        if self.completion.is_none() {
            return false;
        }
        let cond_opt = {
            let completion = self.completion.as_mut().unwrap();
            if !completion.is_visible() {
                return false;
            }
            if let Some(cand) = completion.selected_candidate() {
                completion.reset();
                Some(cand)
            } else {
                None
            }
        };

        if let Some(candidate) = cond_opt {
            self.replace_last_word_part(candidate);
            return true;
        }
        false
    }

    async fn handle_textbox_event(&mut self, key: Key) -> anyhow::Result<EventState> {
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

            Key::Ctrl(KeyCode::Backspace) => {
                let input_str: String = self.input.clone().into_iter().collect();
                if let Some(pos) = sql_utils::find_last_separator(&input_str) {
                    if pos.index + pos.length == self.input_cursor_position {
                        self.input = self.input[0..pos.index].into();
                        self.input_cursor_position = pos.index;
                    } else {
                        self.input = self.input[0..pos.index + pos.length].into();
                        self.input_cursor_position = pos.index + pos.length;
                    }
                } else {
                    self.input.clear();
                    self.input_cursor_position = 0;
                }
                Ok(Consumed)
            }

            Key::Ctrl(KeyCode::Left) => {
                // TODO : Implement ctrl+left and ctrl+right
                Ok(NotConsumed)
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
            Key::Ctrl(event::KeyCode::Char('a')) | Key::Home => {
                self.input_cursor_position = 0;
                Ok(EventState::Consumed)
            }
            Key::Ctrl(event::KeyCode::Char('e')) | Key::End => {
                self.input_cursor_position = self.input.len();
                Ok(EventState::Consumed)
            }
            _ => Ok(NotConsumed),
        };
    }
}

impl DrawableComponent for TextBox {
    fn draw<B: Backend>(&self, f: &mut Frame<B>, area: Rect, focused: bool) -> Result<()> {
        // debug!("Drawing textbox {:?} \nwith area {:?}", self, area);
        if let Some(completion) = &self.completion {
            let (cursor_x, cursor_y) = self.cursor_position(&area);
            completion.draw(f, area, false, cursor_x, cursor_y + 1)?;
        }
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
                self.get_text()
            },
            w = text_rect.width as usize
        )))
        .scroll((
            0,
            if self.input_cursor_position > (text_rect.width as usize) {
                self.input_cursor_position as u16 - text_rect.width
            } else {
                0
            },
        ))
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
        if self.handle_textbox_event(key).await?.is_consumed() {
            // handled key, update text
            if self.completion.is_none() || self.last_word_part().is_none() {
                return Ok(Consumed);
            }
            let last_part = self.last_word_part().unwrap();
            if let Some(c) = self.completion.as_mut() {
                c.update(last_part).await;
            }
            return Ok(Consumed);
        }
        if let Some(comp) = &mut self.completion {
            if comp.event(key, _message_queue).await?.is_consumed() {
                return Ok(Consumed);
            }
        }

        // handle esc, enter, and tab
        return match key {
            Key::Enter | Key::Tab => {
                if self.complete_word().await {
                    Ok(Consumed)
                } else {
                    Ok(NotConsumed)
                }
            }

            Key::Esc => {
                if self.completion.is_none() {
                    return Ok(NotConsumed);
                }
                let completion = self.completion.as_mut().unwrap();
                if !completion.is_visible() {
                    return Ok(NotConsumed);
                }
                completion.reset();
                Ok(Consumed)
            }

            _ => Ok(NotConsumed),
        };
    }
}
