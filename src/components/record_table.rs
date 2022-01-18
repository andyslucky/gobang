use anyhow::Result;
use tui::{
    backend::Backend,
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
};

use database_tree::{Database, Table as DTable};

use crate::clipboard::copy_to_clipboard;
use crate::components::{Drawable, TableComponent, TableFilterComponent};
use crate::components::command::CommandInfo;
use crate::components::databases::{DatabaseEvent, DatabaseMessageObserver};
use crate::components::tab::{Tab, TabType};
use crate::config::KeyConfig;
use crate::event::Key;

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

impl DatabaseMessageObserver for RecordTableComponent {
    fn handle_message(&mut self, message: &DatabaseEvent) -> Result<()> {
       match message {
           DatabaseEvent::TableSelected(_, _) => {
               self.reset();
               // TODO: implmenet rest of logic!
           }
       }
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
    pub fn new(key_config: KeyConfig) -> Self {
        Self {
            filter: TableFilterComponent::new(key_config.clone()),
            table: TableComponent::new(key_config.clone()),
            focus: Focus::Table,
            key_config,
        }
    }

    pub fn update(
        &mut self,
        rows: Vec<Vec<String>>,
        headers: Vec<String>,
        database: Database,
        table: DTable,
    ) {
        self.table.update(rows, headers, database, table.clone());
        self.filter.table = Some(table);
    }

    pub fn reset(&mut self) {
        self.table.reset();
        self.filter.reset();
    }

    pub fn filter_focused(&self) -> bool {
        matches!(self.focus, Focus::Filter)
    }
}

impl Component for RecordTableComponent {
    fn commands(&self, out: &mut Vec<CommandInfo>) {
        self.table.commands(out)
    }

    fn event(&mut self, key: Key) -> Result<EventState> {
        if key == self.key_config.copy {
            if let Some(text) = self.table.selected_cells() {
                copy_to_clipboard(text.as_str())?
            }
        }
        if key == self.key_config.filter {
            self.focus = Focus::Filter;
            return Ok(EventState::Consumed);
        }
        match key {
            key if matches!(self.focus, Focus::Filter) => return self.filter.event(key),
            key if matches!(self.focus, Focus::Table) => return self.table.event(key),
            _ => (),
        }
        Ok(EventState::NotConsumed)
    }
}
