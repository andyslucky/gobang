use anyhow::Result;
use async_trait::async_trait;
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem},
    Frame,
};

use database_tree::{Database, Table};

use crate::app::{AppMessage, AppStateRef};
use crate::clipboard::copy_to_clipboard;
use crate::components::command::{self, CommandInfo};
use crate::components::databases::DatabaseEvent;
use crate::components::tab::{Tab, TabType};
use crate::components::{Drawable, TableComponent};
use crate::config::KeyConfig;
use crate::database::{Column, TableRow};
use crate::handle_message;

use super::{Component, EventState};

#[derive(Debug, PartialEq)]
pub enum Focus {
    Column,
    Constraint,
    ForeignKey,
    Index,
}

impl std::fmt::Display for Focus {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub struct PropertiesComponent {
    column_table: TableComponent,
    constraint_table: TableComponent,
    foreign_key_table: TableComponent,
    index_table: TableComponent,
    focus: Focus,
    key_config: KeyConfig,
    app_state: AppStateRef,
}

impl<B: Backend> Tab<B> for PropertiesComponent {
    fn tab_type(&self) -> TabType {
        TabType::Properties
    }

    fn tab_name(&self) -> String {
        String::from("Properties")
    }
}

impl PropertiesComponent {
    pub fn new(key_config: KeyConfig, app_state: AppStateRef) -> Self {
        Self {
            column_table: TableComponent::new(key_config.clone()),
            constraint_table: TableComponent::new(key_config.clone()),
            foreign_key_table: TableComponent::new(key_config.clone()),
            index_table: TableComponent::new(key_config.clone()),
            focus: Focus::Column,
            key_config,
            app_state,
        }
    }

    fn focused_component(&mut self) -> &mut TableComponent {
        match self.focus {
            Focus::Column => &mut self.column_table,
            Focus::Constraint => &mut self.constraint_table,
            Focus::ForeignKey => &mut self.foreign_key_table,
            Focus::Index => &mut self.index_table,
        }
    }

    async fn update(&mut self, database: Database, table: Table) -> Result<()> {
        self.column_table.reset();
        let mut columns: Vec<Column> = vec![];
        let mut constraints: Vec<Box<dyn TableRow>> = vec![];
        let mut indexes: Vec<Box<dyn TableRow>> = vec![];
        let mut foreign_keys: Vec<Box<dyn TableRow>> = vec![];

        if let Some(pool) = self.app_state.read().await.shared_pool.as_ref() {
            columns = pool.get_columns(&table).await?;
            foreign_keys = pool.get_foreign_keys(&database, &table).await?;
            constraints = pool.get_constraints(&database, &table).await?;
            indexes = pool.get_indexes(&database, &table).await?;
        }

        if !columns.is_empty() {
            self.column_table.update(
                columns
                    .iter()
                    .map(|c| c.columns())
                    .collect::<Vec<Vec<String>>>(),
                columns.get(0).unwrap().fields(),
                database.clone(),
                table.clone(),
            );
        }
        self.constraint_table.reset();
        if !constraints.is_empty() {
            self.constraint_table.update(
                constraints
                    .iter()
                    .map(|c| c.columns())
                    .collect::<Vec<Vec<String>>>(),
                constraints.get(0).unwrap().fields(),
                database.clone(),
                table.clone(),
            );
        }
        self.foreign_key_table.reset();
        if !foreign_keys.is_empty() {
            self.foreign_key_table.update(
                foreign_keys
                    .iter()
                    .map(|c| c.columns())
                    .collect::<Vec<Vec<String>>>(),
                foreign_keys.get(0).unwrap().fields(),
                database.clone(),
                table.clone(),
            );
        }
        self.index_table.reset();
        if !indexes.is_empty() {
            self.index_table.update(
                indexes
                    .iter()
                    .map(|c| c.columns())
                    .collect::<Vec<Vec<String>>>(),
                indexes.get(0).unwrap().fields(),
                database.clone(),
                table.clone(),
            );
        }
        Ok(())
    }

    fn tab_names(&self) -> Vec<(Focus, String)> {
        vec![
            (Focus::Column, command::tab_columns(&self.key_config).name),
            (
                Focus::Constraint,
                command::tab_constraints(&self.key_config).name,
            ),
            (
                Focus::ForeignKey,
                command::tab_foreign_keys(&self.key_config).name,
            ),
            (Focus::Index, command::tab_indexes(&self.key_config).name),
        ]
    }
}

impl<B: Backend> Drawable<B> for PropertiesComponent {
    fn draw(&mut self, f: &mut Frame<B>, area: Rect, focused: bool) -> Result<()> {
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Length(20), Constraint::Min(1)])
            .split(area);

        let tab_names = self
            .tab_names()
            .iter()
            .map(|(f, c)| {
                ListItem::new(c.to_string()).style(if *f == self.focus {
                    Style::default().bg(Color::Rgb(0xea, 0x59, 0x0b))
                } else {
                    Style::default()
                })
            })
            .collect::<Vec<ListItem>>();

        let tab_list = List::new(tab_names)
            .block(Block::default().borders(Borders::ALL).style(if focused {
                Style::default()
            } else {
                Style::default().fg(Color::DarkGray)
            }))
            .style(Style::default());

        f.render_widget(tab_list, layout[0]);

        self.focused_component().draw(f, layout[1], focused)?;
        Ok(())
    }
}

#[async_trait]
impl Component for PropertiesComponent {
    fn commands(&self, out: &mut Vec<CommandInfo>) {
        out.push(CommandInfo::new(command::toggle_property_tabs(
            &self.key_config,
        )));
    }

    async fn event(
        &mut self,
        key: crate::event::Key,
        message_queue: &mut crate::app::GlobalMessageQueue,
    ) -> Result<EventState> {
        self.focused_component().event(key, message_queue).await?;

        if key == self.key_config.copy {
            if let Some(text) = self.focused_component().selected_cells() {
                copy_to_clipboard(text.as_str())?
            }
        } else if key == self.key_config.tab_columns {
            self.focus = Focus::Column;
        } else if key == self.key_config.tab_constraints {
            self.focus = Focus::Constraint;
        } else if key == self.key_config.tab_foreign_keys {
            self.focus = Focus::ForeignKey;
        } else if key == self.key_config.tab_indexes {
            self.focus = Focus::Index;
        }
        Ok(EventState::NotConsumed)
    }
    async fn handle_messages(&mut self, messages: &Vec<Box<dyn AppMessage>>) -> Result<()> {
        for m in messages.iter() {
            handle_message!(m, DatabaseEvent,
                DatabaseEvent::TableSelected(database,table) => {
                        self.reset();
                        self.update(database.clone(), table.clone()).await?;
                }
            );
        }
        Ok(())
    }
}
