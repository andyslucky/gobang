use anyhow::Result;
use async_trait::async_trait;
use crossterm::event::KeyCode;
use log::debug;
use tui::backend::Backend;
use tui::layout::Rect;
use tui::style::{Color, Style};
use tui::text::{Spans, Text};
use tui::widgets::{Block, Borders, Paragraph};
use tui::Frame;

use crate::app::{AppMessage, AppStateRef, GlobalMessageQueue};
use crate::components::databases::DatabaseEvent;
use crate::components::EventState::{Consumed, NotConsumed};
use crate::components::{CommandInfo, DrawableComponent, EventState, MovableComponent};
use crate::components::{CompletionComponent, Component};
use crate::config::KeyConfig;
use crate::saturating_types::SaturatingU16;
use crate::sql_utils::find_last_separator;
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

    async fn update_completion(&mut self) {
        let col = self.cursor_position.col.0 as usize;
        if let Some(current_line) = self.buffer.get(self.cursor_position.row.0 as usize) {
            if let Some(last_sep) = find_last_separator(&current_line[0..col]) {
                let last_word_part = &current_line[(last_sep.index + last_sep.length)..col];
                self.completion.update(last_word_part).await;
            } else {
                self.completion.update(&current_line[0..col]).await;
            }
        }
    }

    fn replace_last_word(&mut self, candidate: &String) {
        let col = self.cursor_position.col.0 as usize;
        if let Some(current_line) = self.buffer.get_mut(self.cursor_position.row.0 as usize) {
            debug!("Here is the current line {}", current_line);
            if let Some(last_sep) = find_last_separator(&current_line[0..col]) {
                debug!("Last separator is {}", last_sep);
                // let last_word_part = &current_line[(last_sep.index + last_sep.length)..col];
                current_line.drain((last_sep.index + last_sep.length)..col);
                current_line.insert_str(last_sep.index + last_sep.length, candidate);
                self.cursor_position.col.0 =
                    (last_sep.index + last_sep.length) as u16 + candidate.len() as u16;
            } else {
                let r = 0..current_line.len();
                current_line.replace_range(r, candidate.as_str());
                self.cursor_position.col.0 = candidate.len() as u16;
            }
        }
    }

    /// Attempts to remove the character before the current cursor position. If the cursor is at the
    /// beginning of a line, the current line will be appended to the previous line and the cursor will be
    /// moved to the last col of the prev line before any contents were appended.
    /// Returns true if the operation was successful, otherwise returns false.
    fn remove_prev_char(&mut self) -> bool {
        // Nothing to delete if the cursor is at the first col of the first line
        if self.cursor_position.row == 0 && self.cursor_position.col == 0 {
            return false;
        }
        // remove prev char on same line
        if self.cursor_position.col > 0 {
            if let Some(current_line) = self.buffer.get_mut(self.cursor_position.row.0 as usize) {
                current_line.remove((self.cursor_position.col - 1).0 as usize);
                self.cursor_position.col -= 1;
                return true;
            }
        } else {
            // remove new line
            let current_row = self.buffer.remove(self.cursor_position.row.0 as usize);
            self.cursor_position.row -= 1;
            // append current row to previous row
            if let Some(prev_line) = self.buffer.get_mut(self.cursor_position.row.0 as usize) {
                self.cursor_position.col.0 = prev_line.len() as u16;
                prev_line.push_str(current_row.as_str());
                return true;
            }
        }
        return false;
    }

    /// Removes the next character after the cursor position.
    /// Removes the new line if the cursor is at the end of a line, and appends the next line to the current line.
    /// If the cursor is at the end of the document, nothing is deleted. Returns true if the next character was
    /// removed otherwise false.
    fn remove_next_char(&mut self) -> bool {
        let (row, col): (SaturatingU16, SaturatingU16) =
            (self.cursor_position.row, self.cursor_position.col);
        let curr_line_length: u16 = self
            .buffer
            .get(row.0 as usize)
            .map(|l| l.len() as u16)
            .unwrap_or(0);
        // If the cursor is at the last col of the last line, there is nothing to delete.
        if (row.0 as usize) == self.buffer.len()
            && (col.0 as usize) == self.buffer.last().map(|l| l.len()).unwrap_or(0)
        {
            return false;
        }
        // Delete empty line
        if curr_line_length == 0 {
            self.buffer.remove(row.0 as usize);
            self.cursor_position.row = if (row.0 as usize) > self.buffer.len() {
                (self.buffer.len() as u16).into()
            } else {
                row
            };
            return true;
        }
        // delete the next character on the same line
        if col < curr_line_length {
            if let Some(current_line) = self.buffer.get_mut(row.0 as usize) {
                current_line.remove(col.0 as usize);
                return true;
            }
        } else if ((row + 1).0 as usize) < self.buffer.len() {
            // delete the new line
            let next_line = self.buffer.remove((row + 1).0 as usize);
            // Nothing to append if the next line is empty
            if next_line.is_empty() {
                return true;
            }
            if let Some(current_line) = self.buffer.get_mut(row.0 as usize) {
                current_line.push_str(next_line.as_str());
                return true;
            }
        }
        return false;
    }

    /// Inserts a new line at the current cursor position.
    fn insert_new_line(&mut self) {
        let (row, col): (SaturatingU16, SaturatingU16) =
            (self.cursor_position.row, self.cursor_position.col);
        let curr_line_length: u16 = self
            .buffer
            .get(row.0 as usize)
            .map(|l| l.len() as u16)
            .unwrap_or(0);
        let new_line: String = if col < curr_line_length {
            // Move contents after cursor on current line to a new line
            self.buffer
                .get_mut(self.cursor_position.row.0 as usize)
                .map(|l| l.drain(col.0 as usize..l.len()).collect())
                .unwrap_or(String::from(""))
        } else {
            String::new()
        };
        // Insert new row
        self.buffer.insert((row + 1).0 as usize, new_line);
        self.cursor_position.col = 0.into();
        self.cursor_position.row += 1;
    }

    fn remove_prev_word(&mut self) -> bool {
        // TODO: Implement Ctrl + Backspace
        false
    }

    fn remove_next_word(&mut self) -> bool {
        // TODO: Implement Ctrl + Del
        false
    }

    // TODO : Move Home, Esc, Ctrl + Home, and Ctrl + Esc here
    fn move_to_end_of_line(&mut self) -> bool {
        false
    }

    fn move_to_beginning_of_line(&mut self) -> bool {
        false
    }

    fn move_to_end_of_doc(&mut self) -> bool {
        false
    }

    fn move_to_beginning_of_doc(&mut self) -> bool {
        false
    }

    /// Autocompletes the current word with the selected candidate from the completion component.
    /// Returns true if there is a candidate selected otherwise false.
    fn complete_word(&mut self) -> bool {
        if let Some(cand) = self.completion.selected_candidate() {
            debug!("Here is the candidate for textarea completion {}", cand);
            self.replace_last_word(&cand);
            self.completion.reset();
            return true;
        }
        return false;
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
                if row >= text_area_frame.height {
                    (row - text_area_frame.height) + 1
                } else {
                    0
                },
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
            let cursor_x = if col + text_area_frame.left() >= text_area_frame.right() {
                text_area_frame.right()
            } else {
                col + text_area_frame.left()
            };

            let cursor_y = if row + text_area_frame.top() >= text_area_frame.bottom() {
                text_area_frame.bottom() - 1
            } else {
                row + text_area_frame.top()
            };
            f.set_cursor(cursor_x, cursor_y);
        }
        self.completion.draw(
            f,
            text_area_frame,
            false,
            text_area_frame.x + self.cursor_position.col.0,
            text_area_frame.y + (self.cursor_position.row + 1).0,
        )?;
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
        if self
            .completion
            .event(key, _message_queue)
            .await?
            .is_consumed()
        {
            return Ok(Consumed);
        }
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
            self.update_completion().await;
            return Ok(Consumed);
        }

        if (key == Key::Enter || key == Key::Tab) && self.completion.is_visible() {
            if self.complete_word() {
                return Ok(Consumed);
            }
        }

        if key == Key::Enter {
            self.insert_new_line();
            self.completion.reset();
            return Ok(Consumed);
        }

        if key == Key::Delete {
            if self.remove_next_char() {
                self.update_completion().await;
                return Ok(Consumed);
            }
        }

        if key == Key::Home {
            self.cursor_position.col.0 = 0;
            return Ok(Consumed);
        }

        if key == Key::Ctrl(KeyCode::Home) {
            self.cursor_position.row.0 = 0;
            self.cursor_position.col.0 = 0;
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
            self.cursor_position.col.0 = curr_line_length;
            return Ok(Consumed);
        }

        if key == Key::Backspace {
            if self.remove_prev_char() {
                self.update_completion().await;
                return Ok(Consumed);
            }
        }

        if key == Key::Left {
            self.completion.reset();
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
            self.completion.reset();
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
            self.completion.reset();
            if row > 0 {
                self.cursor_position.row -= 1;
                if col > last_line_length {
                    self.cursor_position.col = last_line_length.into();
                }
                return Ok(Consumed);
            }
        }

        if key == Key::Down {
            self.completion.reset();
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
