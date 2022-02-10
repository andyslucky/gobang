use anyhow::Result;
use async_trait::async_trait;
use log::info;
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use unicode_width::UnicodeWidthStr;

use crate::app::{AppMessage, AppStateRef, GlobalMessageQueue};
use crate::components::command::CommandInfo;
use crate::components::databases::DatabaseEvent;
use crate::components::tab::{Tab, TabType};
use crate::components::Drawable;
use crate::components::EventState::{Consumed, NotConsumed};
use crate::config::KeyConfig;
use crate::database::ExecuteResult;
use crate::event::Key;
use crate::handle_message;
use crate::sql_utils::find_last_separator;
use crate::ui::stateful_paragraph::{ParagraphState, StatefulParagraph};

use super::{
    compute_character_width, CompletionComponent, Component, EventState, MovableComponent,
    TableComponent,
};

struct QueryResult {
    updated_rows: u64,
}

impl QueryResult {
    fn result_str(&self) -> String {
        format!("Query OK, {} row affected", self.updated_rows)
    }
}

pub enum Focus {
    Editor,
    Table,
}

pub struct SqlEditorComponent {
    input: Vec<char>,
    input_cursor_position_x: u16,
    input_idx: usize,
    table: TableComponent,
    query_result: Option<QueryResult>,
    completion: CompletionComponent,
    key_config: KeyConfig,
    paragraph_state: ParagraphState,
    focus: Focus,
    app_state: AppStateRef,
    editor_name: String,
}

impl<B: Backend> Tab<B> for SqlEditorComponent {
    fn tab_type(&self) -> TabType {
        TabType::Sql
    }

    fn tab_name(&self) -> String {
        self.editor_name.clone()
    }

    fn update_name(&mut self, name: String) {
        self.editor_name = name;
    }
}

impl SqlEditorComponent {
    pub async fn new(
        key_config: KeyConfig,
        app_state: AppStateRef,
        editor_name: Option<String>,
    ) -> Self {
        let mut completion = CompletionComponent::new(key_config.clone());
        if let Some(src) = app_state.clone().read().await.pool_completion_src().await {
            completion.completion_source = Box::new(src);
        }
        Self {
            input: Vec::new(),
            input_idx: 0,
            input_cursor_position_x: 0,
            table: TableComponent::new(key_config.clone()),
            completion,
            focus: Focus::Editor,
            paragraph_state: ParagraphState::default(),
            query_result: None,
            key_config,
            app_state,
            editor_name: editor_name.unwrap_or("Sql Editor".to_string()),
        }
    }

    fn last_word_part(&self) -> String {
        let input: String = self.input.clone().into_iter().collect();
        if let Some(pos) = find_last_separator(&input) {
            return self.input[(pos.index + pos.length)..self.input_idx]
                .iter()
                .collect();
        }
        input
    }

    fn complete(&mut self) {
        // TODO : Cleanup editor code before implementing completion!
        info!("TODO: reimplement completion!");
        // if let Some(_) = self.completion.selected_candidate() {}
        self.completion.reset();
    }

    async fn editor_key_event(
        &mut self,
        key: Key,
        _: &mut GlobalMessageQueue,
    ) -> Result<EventState> {
        match key {
            Key::Char(c) => {
                self.input.insert(self.input_idx, c);
                self.input_idx += 1;
                self.input_cursor_position_x += compute_character_width(&c);
                let last_w = self.last_word_part();
                self.completion.update(last_w).await;
                return Ok(EventState::Consumed);
            }
            Key::Enter => {
                if self.completion.is_visible() {
                    self.complete();
                    return Ok(Consumed);
                } else {
                    // TODO : Implement enter key
                    self.input.insert(self.input_idx, '\n');
                    self.input_idx += 1;
                    self.input_cursor_position_x = 0;
                    return Ok(Consumed);
                }
            }

            Key::Tab => {
                if self.completion.is_visible() {
                    self.complete();
                    return Ok(Consumed);
                }
            }
            Key::Esc => {
                self.focus = Focus::Table;
                return Ok(EventState::Consumed);
            }
            Key::Backspace => {
                let input_str: String = self.input.iter().collect();
                if input_str.width() > 0 && !self.input.is_empty() && self.input_idx > 0 {
                    let last_c = self.input.remove(self.input_idx - 1);
                    self.input_idx -= 1;
                    if last_c == '\n' {
                        let mut x_offset = 0;
                        for c in self.input.iter().rev() {
                            if *c == '\n' {
                                break;
                            }
                            x_offset += compute_character_width(c);
                        }
                        self.input_cursor_position_x = x_offset;
                    } else {
                        self.input_cursor_position_x -= compute_character_width(&last_c);
                    }
                    // self.completion.update("");
                }
                return Ok(EventState::Consumed);
            }
            Key::Left => {
                if !self.input.is_empty() && self.input_idx > 0 {
                    self.input_idx -= 1;
                    self.input_cursor_position_x = self
                        .input_cursor_position_x
                        .saturating_sub(compute_character_width(&self.input[self.input_idx]));
                }
                return Ok(EventState::Consumed);
            }
            Key::Right => {
                if self.input_idx < self.input.len() {
                    let next_c = self.input[self.input_idx];
                    self.input_idx += 1;
                    self.input_cursor_position_x += compute_character_width(&next_c);
                }
                return Ok(EventState::Consumed);
            }
            Key::F5 => {
                let query: String = self.input.iter().collect();
                self.execute_query(query).await?;
                return Ok(EventState::Consumed);
            }
            _ => (),
        }
        Ok(NotConsumed)
    }

    async fn execute_query(&mut self, query: String) -> Result<()> {
        if let Some(pool) = self.app_state.read().await.shared_pool.as_ref() {
            let result = pool.execute(&query).await?;
            match result {
                ExecuteResult::Read {
                    headers,
                    rows,
                    database,
                    table,
                } => {
                    self.table.update(rows, headers, database, table);
                    self.focus = Focus::Table;
                    self.query_result = None;
                }
                ExecuteResult::Write { updated_rows } => {
                    self.query_result = Some(QueryResult { updated_rows })
                }
            }
        }
        Ok(())
    }
}

impl<B: Backend> Drawable<B> for SqlEditorComponent {
    fn draw(&mut self, f: &mut Frame<B>, area: Rect, focused: bool) -> Result<()> {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(if matches!(self.focus, Focus::Table) {
                vec![Constraint::Length(7), Constraint::Min(1)]
            } else {
                vec![Constraint::Percentage(50), Constraint::Min(1)]
            })
            .split(area);

        let editor = StatefulParagraph::new(self.input.iter().collect::<String>())
            .wrap(Wrap { trim: true })
            .block(Block::default().borders(Borders::ALL))
            .style(if focused {
                Style::default()
            } else {
                Style::default().fg(Color::DarkGray)
            });

        f.render_stateful_widget(editor, layout[0], &mut self.paragraph_state);

        if let Some(result) = self.query_result.as_ref() {
            let result = Paragraph::new(result.result_str())
                .block(Block::default().borders(Borders::ALL).style(
                    if focused && matches!(self.focus, Focus::Editor) {
                        Style::default()
                    } else {
                        Style::default().fg(Color::DarkGray)
                    },
                ))
                .wrap(Wrap { trim: true });
            f.render_widget(result, layout[1]);
        } else {
            self.table
                .draw(f, layout[1], focused && matches!(self.focus, Focus::Table))?;
        }

        let lines = self.input.iter().filter(|c| **c == '\n').count();
        if focused && matches!(self.focus, Focus::Editor) {
            f.set_cursor(
                (layout[0].x + 1)
                    .saturating_add(
                        self.input_cursor_position_x % layout[0].width.saturating_sub(2),
                    )
                    .min(area.right().saturating_sub(2)),
                (layout[0].y
                    + 1
                    + lines as u16
                    + self.input_cursor_position_x / layout[0].width.saturating_sub(2))
                .min(layout[0].bottom()),
            )
        }

        if focused && matches!(self.focus, Focus::Editor) {
            self.completion.draw(
                f,
                area,
                false,
                self.input_cursor_position_x % layout[0].width.saturating_sub(2) + 1,
                self.input_cursor_position_x / layout[0].width.saturating_sub(2),
            )?;
        };
        Ok(())
    }
}

#[async_trait]
impl Component for SqlEditorComponent {
    fn commands(&self, _out: &mut Vec<CommandInfo>) {}

    async fn event(
        &mut self,
        key: crate::event::Key,
        message_queue: &mut GlobalMessageQueue,
    ) -> Result<EventState> {
        // if key == self.key_config.focus_above && matches!(self.focus, Focus::Table) {
        //     self.focus = Focus::Editor
        // } else if key == self.key_config.enter {
        //     return self.complete();
        // }

        return match self.focus {
            Focus::Editor => self.editor_key_event(key, message_queue).await,
            Focus::Table => {
                if key == self.key_config.focus_above {
                    self.focus = Focus::Editor;
                    return Ok(EventState::Consumed);
                }
                self.table.event(key, message_queue).await
            }
        };
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
