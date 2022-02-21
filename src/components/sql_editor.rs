use anyhow::Result;
use async_trait::async_trait;
use log::info;
use tui::{
    backend::Backend,
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use unicode_width::UnicodeWidthStr;

use crate::app::{AppMessage, AppStateRef, GlobalMessageQueue};
use crate::components::{Drawable, DrawableComponent};
use crate::components::command::CommandInfo;
use crate::components::databases::DatabaseEvent;
use crate::components::EventState::{Consumed, NotConsumed};
use crate::components::tab::{Tab, TabType};
use crate::config::KeyConfig;
use crate::database::ExecuteResult;
use crate::event::Key;
use crate::handle_message;
use crate::sql_utils::find_last_separator;
use crate::ui::stateful_paragraph::{ParagraphState, StatefulParagraph};
use crate::ui::textarea::TextArea;

use super::{
    CompletionComponent, Component, compute_character_width, EventState, MovableComponent,
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
    text_area: TextArea,
    table: TableComponent,
    query_result: Option<QueryResult>,
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
        // TODO: Move into text editor component
       
        Self {
            text_area: TextArea::new(key_config.clone(), app_state.clone()).await,
            table: TableComponent::new(key_config.clone()),
            focus: Focus::Editor,
            paragraph_state: ParagraphState::default(),
            query_result: None,
            key_config,
            app_state,
            editor_name: editor_name.unwrap_or("Sql Editor".to_string()),
        }
    }

    async fn editor_key_event(
        &mut self,
        key: Key,
        msg_queue: &mut GlobalMessageQueue,
    ) -> Result<EventState> {
        if self.text_area.event(key, msg_queue).await?.is_consumed() {
            return Ok(Consumed);
        }
        match key {
            Key::Esc => {
                self.focus = Focus::Table;
                return Ok(EventState::Consumed);
            }
            Key::F5 => {
                let query: String = self.text_area.get_text();
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
        self.text_area
            .draw(f, layout[0], focused && matches!(self.focus, Focus::Editor))?;

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
}
