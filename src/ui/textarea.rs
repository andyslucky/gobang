use anyhow::Result;

use async_trait::async_trait;
use crossterm::event::KeyCode;
use tui::backend::Backend;
use tui::layout::Rect;
use tui::style::{Color, Style};
use tui::text::{Spans, Text};
use tui::widgets::{Block, Borders, Paragraph};
use tui::Frame;

use crate::app::{AppMessage, AppStateRef, GlobalMessageQueue};
use crate::components::databases::DatabaseEvent;
use crate::components::EventState::{Consumed, NotConsumed};
use crate::components::{CommandInfo, DrawableComponent, EventState};
use crate::components::{CompletionComponent, Component};
use crate::config::KeyConfig;
use crate::saturating_types::SaturatingU16;
use crate::{handle_message, Key};

#[derive(Clone)]
struct CursorPos {
    row: SaturatingU16,
    col: SaturatingU16,
}

impl Into<(u16, u16)> for CursorPos {
    fn into(self) -> (u16, u16) {
        (self.row.into(), self.col.into())
    }
}

pub struct TextArea {
    buffer: Vec<String>,
    app_state: AppStateRef,
    completion: CompletionComponent,
    cursor_position: CursorPos,
}

impl TextArea {
    pub async fn new(key_config: KeyConfig, app_state: AppStateRef) -> TextArea {
        let mut completion = CompletionComponent::new(key_config.clone());
        if let Some(src) = app_state.clone().read().await.pool_completion_src().await {
            completion.completion_source = Box::new(src);
        }
        return TextArea {
            buffer: Vec::new(),
            completion,
            app_state,
            cursor_position: CursorPos {
                row: 0.into(),
                col: 0.into(),
            },
        };
    }

    pub fn get_text(&self) -> String {
        return self.buffer.join("\n");
    }

    /// Get input as vec of spans converted into text. Each 'Spans' element is composed of multiple
    /// graphemes each with their own symbol, style, and modifiers. Text encapsulates a vec of spans;
    /// so we convert the vector of spans into a text before returning.
    fn lines_as_text_model(&self) -> Text {
        // TODO : Add different styling/highlights to keywords
        // let lines: Vec<Spans> = self.buffer.split('\n').map(|l| Spans::from(l)).collect();
        let lines: Vec<Spans> = self
            .buffer
            .iter()
            .map(|l| Spans::from(l.as_str()))
            .collect();
        Text::from(lines)
    }
}

impl DrawableComponent for TextArea {
    fn draw<B: Backend>(&self, f: &mut Frame<B>, area: Rect, focused: bool) -> anyhow::Result<()> {
        let row = self.cursor_position.row.0;
        let col = self.cursor_position.col.0;
        let block = Block::default().borders(Borders::ALL).style(if focused {
            Style::default()
        } else {
            Style::default().fg(Color::DarkGray)
        });
        let text_area_frame = block.inner(area);
        f.render_widget(block, area);

        let p = Paragraph::new(self.lines_as_text_model())
            .scroll((
                0, // TODO: Fix row scrolling
                if col > text_area_frame.width {
                    col - text_area_frame.width
                } else {
                    0
                },
            ))
            .style(if !focused {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default()
            });
        f.render_widget(p, text_area_frame);
        if focused {
            f.set_cursor(
                text_area_frame.x
                    + if col > text_area_frame.width {
                        text_area_frame.width
                    } else {
                        col
                    },
                text_area_frame.y
                    + if row > text_area_frame.height {
                        text_area_frame.height
                    } else {
                        row
                    },
            );
        }
        Ok(())
    }
}

#[async_trait]
impl Component for TextArea {
    fn commands(&self, out: &mut Vec<CommandInfo>) {}

    async fn event(
        &mut self,
        key: Key,
        _message_queue: &mut GlobalMessageQueue,
    ) -> anyhow::Result<EventState> {
        // TODO: Move this logic to a TextAreaModel
        let col = self.cursor_position.col.clone();
        let row = self.cursor_position.row.clone();
        let curr_line_length = self
            .buffer
            .get(row.0 as usize)
            .map(|l| l.len() as u16)
            .unwrap_or(0);
        let last_line_length = self
            .buffer
            .get((row - 1).0 as usize)
            .map(|l| l.len() as u16)
            .unwrap_or(0);

        if let Key::Char(c) = key {
            let current_line: &mut String = {
                //get current line (corresponds to the
                if let Some(line) = self.buffer.get_mut(row.0 as usize) {
                    line
                } else {
                    self.buffer.push(String::new());
                    self.buffer.last_mut().unwrap()
                }
            };
            current_line.insert(col.0 as usize, c);

            self.cursor_position.col += 1;
            return Ok(Consumed);
        }

        if key == Key::Enter {
            let mut new_line: String;
            if col < curr_line_length {
                let line_remainder = self
                    .buffer
                    .get_mut(row.0 as usize)
                    .map(|l| l.drain(col.0 as usize..l.len()).collect())
                    .unwrap_or(String::from(""));
                new_line = line_remainder;
            } else {
                new_line = String::new();
            }
            self.buffer.insert((row + 1).0 as usize, new_line);
            self.cursor_position.col = 0.into();
            self.cursor_position.row += 1;
            return Ok(Consumed);
        }

        if key == Key::Delete {
            if col < curr_line_length {
                if let Some(current_line) = self.buffer.get_mut(row.0 as usize) {
                    current_line.remove(col.0 as usize);
                    return Ok(Consumed);
                }
            } else {
                // TODO : Handle line wrapping del
            }
        }

        if key == Key::Home && col != 0 {
            self.cursor_position.col = 0.into();
            return Ok(Consumed);
        }

        if key == Key::Ctrl(KeyCode::Home) {
            self.cursor_position.row = 0.into();
            self.cursor_position.col = 0.into();
            return Ok(Consumed);
        }

        if key == Key::Ctrl(KeyCode::End) {
            self.cursor_position.row = ((self.buffer.len() - 1) as u16).into();
            self.cursor_position.col = self
                .buffer
                .last()
                .map(|l| l.len() as u16)
                .unwrap_or(0)
                .into();
            return Ok(Consumed);
        }
        if key == Key::End && col < curr_line_length {
            self.cursor_position.col = curr_line_length.into();
            return Ok(Consumed);
        }

        if key == Key::Backspace {
            if col == 0 && (row - 1) < last_line_length {
                let current_line = self.buffer.pop().unwrap_or(String::new());
                if let Some(prev_line) = self.buffer.get_mut((row - 1).0 as usize) {
                    prev_line.insert_str(last_line_length as usize, current_line.as_str());
                    self.cursor_position.col = last_line_length.into();
                    self.cursor_position.row -= 1;
                    return Ok(Consumed);
                }
            } else if let Some(current_line) = self.buffer.get_mut(row.0 as usize) {
                if current_line.is_empty() {
                    self.buffer.pop();
                    if let Some(prev_line_length) = self.buffer.last().map(|l| l.len() as u16) {
                        self.cursor_position.col = prev_line_length.into();
                        self.cursor_position.row -= 1;
                    }
                    return Ok(Consumed);
                } else if ((col - 1).0 as usize) < current_line.len() {
                    current_line.remove((col - 1).0 as usize);
                    self.cursor_position.col -= 1;
                    return Ok(Consumed);
                }
            }
        }

        if key == Key::Left {
            if col == 0 && row > 0 {
                self.cursor_position.col = self
                    .buffer
                    .get((row - 1).0 as usize)
                    .map(|l| l.len() as u16)
                    .unwrap_or(0)
                    .into();
                self.cursor_position.row -= 1;
                return Ok(Consumed);
            } else if col > 0 {
                self.cursor_position.col -= 1;
                return Ok(Consumed);
            }
        }

        if key == Key::Right {
            if col == curr_line_length && (row.0 as usize) < self.buffer.len().saturating_sub(1) {
                self.cursor_position.col = 0.into();
                self.cursor_position.row += 1;
                return Ok(Consumed);
            } else if col < curr_line_length {
                self.cursor_position.col += 1;
                return Ok(Consumed);
            }
        }

        if key == Key::Up {
            if row > 0 {
                self.cursor_position.row -= 1;

                if col > last_line_length {
                    self.cursor_position.col = last_line_length.into();
                }
                return Ok(Consumed);
            }
        }

        if key == Key::Down {
            if (row.0 as usize) < (self.buffer.len().saturating_sub(1)) {
                self.cursor_position.row += 1;
                let last_line_length = self
                    .buffer
                    .get(row.0 as usize)
                    .map(|l| l.len() as u16)
                    .unwrap_or(0);
                if col > last_line_length {
                    self.cursor_position.col = last_line_length.into();
                }
                return Ok(Consumed);
            }
        }

        Ok(NotConsumed)
    }

    async fn handle_messages(&mut self, messages: &Vec<Box<dyn AppMessage>>) -> Result<()> {
        for m in messages.iter() {
            handle_message!(m,DatabaseEvent, DatabaseEvent::TableSelected(_, _) => {

                if let Some(src) = self.app_state.read().await.pool_completion_src().await {
                    self.completion.completion_source = Box::new(src);
                }
            });
        }
        Ok(())
    }
}
