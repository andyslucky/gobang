use anyhow::Result;
use async_trait::async_trait;
use tui::{
    backend::Backend,
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use unicode_width::UnicodeWidthStr;

use crate::app::{GlobalMessageQueue, SharedPool};
use crate::components::command::CommandInfo;
use crate::components::databases::DatabaseEvent;
use crate::components::Drawable;
use crate::components::EventState::NotConsumed;
use crate::components::tab::{Tab, TabType};
use crate::config::KeyConfig;
use crate::database::{ExecuteResult, Pool};
use crate::event::Key;
use crate::ui::stateful_paragraph::{ParagraphState, StatefulParagraph};

use super::{
    CompletionComponent, Component, compute_character_width, EventState, MovableComponent, TableComponent,
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
    shared_pool : SharedPool
}

impl<B : Backend> Tab<B> for SqlEditorComponent {
    fn tab_type(&self) -> TabType {
        TabType::Sql
    }

    fn tab_name(&self) -> String {
        String::from("Sql Editor")
    }
}


impl SqlEditorComponent {
    pub fn new(key_config: KeyConfig, shared_pool : SharedPool) -> Self {
        Self {
            input: Vec::new(),
            input_idx: 0,
            input_cursor_position_x: 0,
            table: TableComponent::new(key_config.clone()),
            completion: CompletionComponent::new(key_config.clone(), "", true),
            focus: Focus::Editor,
            paragraph_state: ParagraphState::default(),
            query_result: None,
            key_config,
            shared_pool
        }
    }

    fn update_completion(&mut self) {
        let input = &self
            .input
            .iter()
            .enumerate()
            .filter(|(i, _)| i < &self.input_idx)
            .map(|(_, i)| i)
            .collect::<String>()
            .split(' ')
            .map(|i| i.to_string())
            .collect::<Vec<String>>();
        self.completion
            .update(input.last().unwrap_or(&String::new()));
    }

    fn complete(&mut self) -> anyhow::Result<EventState> {
        if let Some(candidate) = self.completion.selected_candidate() {
            let mut input = Vec::new();
            let first = self
                .input
                .iter()
                .enumerate()
                .filter(|(i, _)| i < &self.input_idx.saturating_sub(self.completion.word().len()))
                .map(|(_, c)| c.to_string())
                .collect::<Vec<String>>();
            let last = self
                .input
                .iter()
                .enumerate()
                .filter(|(i, _)| i >= &self.input_idx)
                .map(|(_, c)| c.to_string())
                .collect::<Vec<String>>();

            let is_last_word = last.first().map_or(false, |c| c == &" ".to_string());

            let middle = if is_last_word {
                candidate
                    .chars()
                    .map(|c| c.to_string())
                    .collect::<Vec<String>>()
            } else {
                let mut c = candidate
                    .chars()
                    .map(|c| c.to_string())
                    .collect::<Vec<String>>();
                c.push(" ".to_string());
                c
            };

            input.extend(first);
            input.extend(middle.clone());
            input.extend(last);

            self.input = input.join("").chars().collect();
            self.input_idx += &middle.len();
            if is_last_word {
                self.input_idx += 1;
            }
            self.input_idx -= self.completion.word().len();
            self.input_cursor_position_x += middle
                .join("")
                .chars()
                .map(compute_character_width)
                .sum::<u16>();
            if is_last_word {
                self.input_cursor_position_x += " ".to_string().width() as u16
            }
            self.input_cursor_position_x -= self
                .completion
                .word()
                .chars()
                .map(compute_character_width)
                .sum::<u16>();
            self.update_completion();
            return Ok(EventState::Consumed);
        }
        Ok(EventState::NotConsumed)
    }


    async fn editor_key_event(&mut self, key : Key, _ :&mut GlobalMessageQueue) -> Result<EventState>{
        match key {
            Key::Char(c) => {
                self.input.insert(self.input_idx, c);
                self.input_idx += 1;
                self.input_cursor_position_x += compute_character_width(c);
                self.update_completion();

                return Ok(EventState::Consumed);
            },
            Key::Enter => {
               // TODO : Implement enter key
            },
            Key::Esc => {
                self.focus = Focus::Table;
                return Ok(EventState::Consumed);
            },
            Key::Delete | Key::Backspace => {
                let input_str: String = self.input.iter().collect();
                if input_str.width() > 0 && !self.input.is_empty() && self.input_idx > 0 {
                    let last_c = self.input.remove(self.input_idx - 1);
                    self.input_idx -= 1;
                    self.input_cursor_position_x -= compute_character_width(last_c);
                    self.completion.update("");
                }
                return Ok(EventState::Consumed);
            },
            Key::Left  => {
                if !self.input.is_empty() && self.input_idx > 0 {
                    self.input_idx -= 1;
                    self.input_cursor_position_x = self
                        .input_cursor_position_x
                        .saturating_sub(compute_character_width(self.input[self.input_idx]));
                    self.completion.update("");
                }
                return Ok(EventState::Consumed);
            }
            Key::Right  => {
                if self.input_idx < self.input.len() {
                    let next_c = self.input[self.input_idx];
                    self.input_idx += 1;
                    self.input_cursor_position_x += compute_character_width(next_c);
                    self.completion.update("");
                }
                return Ok(EventState::Consumed);
            },
            Key::F10 => {
                    let query : String = self.input.iter().collect();
                    self.update_table(query).await?;
                    return Ok(EventState::Consumed);
            },
            _ => ()
        }
        Ok(NotConsumed)
    }

    async fn update_table(&mut self, query: String) -> Result<()> {
        if let Some(pool) = self.shared_pool.read().await.as_ref() {
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

impl<B : Backend> Drawable<B> for SqlEditorComponent {
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
            .style(if focused {Style::default()} else {Style::default().fg(Color::DarkGray)});

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

        if focused && matches!(self.focus, Focus::Editor) {
            f.set_cursor(
                (layout[0].x + 1)
                    .saturating_add(
                        self.input_cursor_position_x % layout[0].width.saturating_sub(2),
                    )
                    .min(area.right().saturating_sub(2)),
                (layout[0].y
                    + 1
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

    async fn event(&mut self, key: crate::event::Key, message_queue: &mut GlobalMessageQueue) -> Result<EventState> {


        // if key == self.key_config.focus_above && matches!(self.focus, Focus::Table) {
        //     self.focus = Focus::Editor
        // } else if key == self.key_config.enter {
        //     return self.complete();
        // }

        match self.focus {
            Focus::Editor => {
                return self.editor_key_event(key,message_queue).await;
            }
            Focus::Table => {
                if key == self.key_config.focus_above {
                    self.focus = Focus::Editor;
                    return Ok(EventState::Consumed);
                }
                return self.table.event(key, message_queue).await;
            }
        }
    }
}
