use anyhow::Result;
use async_trait::async_trait;
use tui::{
    backend::Backend,
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
};

use database_tree::{Database, Table as DTable};

use crate::app::{AppMessage, GlobalMessageQueue, SharedPool};
use crate::clipboard::copy_to_clipboard;
use crate::components::{Drawable, TableComponent, TableFilterComponent};
use crate::components::command::CommandInfo;
use crate::components::databases::DatabaseEvent;
use crate::components::databases::DatabaseEvent::TableSelected;
use crate::components::tab::{Tab, TabType};
use crate::config::KeyConfig;
use crate::{handle_message, Key};

use super::{Component, EventState};

pub enum Focus {
    Table,
    Filter,
}

pub struct RecordTableComponent {
    pub filter: TableFilterComponent,
    pub table: TableComponent,
    pub focus: Focus,
    key_config: KeyConfig,
    shared_pool : SharedPool
}

impl<B: Backend> Drawable<B> for RecordTableComponent {
    fn draw(&mut self, f: &mut Frame<B>, area: Rect, focused: bool) -> Result<()> {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Length(3), Constraint::Length(5)])
            .split(area);

        self.table
            .draw(f, layout[1], focused && matches!(self.focus, Focus::Table))?;

        self.filter
            .draw(f, layout[0], focused && matches!(self.focus, Focus::Filter))?;
        Ok(())
    }
}

impl<B : Backend> Tab<B> for RecordTableComponent {
    fn tab_type(&self) -> TabType {
        TabType::Records
    }

    fn tab_name(&self) -> String {
        String::from("Records")
    }


}

impl RecordTableComponent {
    pub fn new(key_config: KeyConfig, shared_pool : SharedPool) -> Self {
        Self {
            filter: TableFilterComponent::new(key_config.clone()),
            table: TableComponent::new(key_config.clone()),
            focus: Focus::Table,
            key_config,
            shared_pool
        }
    }

    async fn update_table(
        &mut self,
        database: Database,
        table: DTable,
    ) -> Result<()> {

        let mut headers : Vec<String> = vec![];
        let mut rows : Vec<Vec<String>> = vec![];
        if let Some(pool) = self.shared_pool.read().await.as_ref() {
            let filter = self.filter.input_str();
            let res = pool
                .get_records(&database, &table, 0, if filter.is_empty() {None} else {Some(filter)})
                .await?;
            headers = res.0;
            rows = res.1;
        }
        self.table.update(rows, headers, database, table.clone());
        self.filter.table = Some(table);
        Ok(())
    }

    fn reset(&mut self) {
        self.table.reset();
        self.filter.reset();
    }
}

#[async_trait]
impl Component for RecordTableComponent {
    fn commands(&self, out: &mut Vec<CommandInfo>) {
        self.table.commands(out)
    }

    async fn event(&mut self, key: Key, message_queue: &mut GlobalMessageQueue) -> Result<EventState> {
        if key == self.key_config.copy {
            if let Some(text) = self.table.selected_cells() {
                copy_to_clipboard(text.as_str())?
            }
        }
        if key == self.key_config.filter {
            self.focus = Focus::Filter;
            return Ok(EventState::Consumed);
        }

        return match self.focus {
            Focus::Table => {
                self.table.event(key, message_queue).await
            }
            Focus::Filter => {
                self.filter.event(key, message_queue).await
            }
        };
    }

    async fn handle_messages(&mut self, messages: &Vec<Box<dyn AppMessage>>) -> Result<()> {
        for m in messages.iter() {
            handle_message!(m, DatabaseEvent,
                TableSelected(database,table) => {
                    self.reset();
                    self.update_table(database.clone(), table.clone()).await?;
                }
            );
        }
        // TODO : Add filter message handling
        Ok(())
    }
}
