use anyhow::Result;
use async_trait::async_trait;
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    Frame,
};

use database_tree::{Database, Table as DTable};

use crate::app::{AppMessage, AppStateRef, GlobalMessageQueue};
use crate::components::command::CommandInfo;
use crate::components::databases::DatabaseEvent;
use crate::components::databases::DatabaseEvent::TableSelected;
use crate::components::tab::{Tab, TabType};
use crate::components::EventState::{Consumed, NotConsumed};
use crate::components::{Drawable, DrawableComponent, TableComponent};
use crate::config::KeyConfig;
use crate::ui::textbox::TextBox;
use crate::{handle_message, Key};

use super::{Component, EventState};

pub enum Focus {
    Table,
    Filter,
}

pub struct RecordTableComponent {
    pub filter: TextBox,
    pub table: TableComponent,
    pub focus: Focus,
    key_config: KeyConfig,
    database: Option<Database>,
    dtable: Option<DTable>,
    app_state: AppStateRef,
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

impl<B: Backend> Tab<B> for RecordTableComponent {
    fn tab_type(&self) -> TabType {
        TabType::Records
    }

    fn tab_name(&self) -> String {
        String::from("Records")
    }
}

impl RecordTableComponent {
    pub fn new(key_config: KeyConfig, app_state: AppStateRef) -> Self {
        Self {
            filter: TextBox::default()
                .with_placeholder("Enter SQL expression to filter records")
                .with_completion(key_config.clone()),
            table: TableComponent::new(key_config.clone()),
            focus: Focus::Table,
            key_config,
            app_state,
            database: None,
            dtable: None,
        }
    }

    async fn update_table(&mut self, database: Database, table: DTable) -> Result<()> {
        self.database = Some(database);
        self.dtable = Some(table);
        self.reload_results_table().await
    }

    async fn reload_results_table(&mut self) -> Result<()> {
        if let Some(database) = &self.database {
            if let Some(table) = &self.dtable {
                let mut headers: Vec<String> = vec![];
                let mut rows: Vec<Vec<String>> = vec![];
                if let Some(pool) = self.app_state.read().await.shared_pool.as_ref() {
                    let filter = self.filter.get_text();
                    let res = pool
                        .get_records(
                            database,
                            table,
                            0,
                            if filter.is_empty() {
                                None
                            } else {
                                Some(filter)
                            },
                        )
                        .await?;
                    headers = res.0;
                    rows = res.1;
                }
                self.table
                    .update(rows, headers, database.clone(), table.clone());
                self.filter.set_label(table.clone().name);
                // self.filter.set_table(table.clone());
            }
        }

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

    async fn event(
        &mut self,
        key: Key,
        message_queue: &mut GlobalMessageQueue,
    ) -> Result<EventState> {
        return match self.focus {
            Focus::Table => {
                if key == self.key_config.filter {
                    self.focus = Focus::Filter;
                    Ok(EventState::Consumed)
                } else {
                    self.table.event(key, message_queue).await
                }
            }
            Focus::Filter => {
                if self.filter.event(key, message_queue).await?.is_consumed() {
                    Ok(Consumed)
                } else {
                    if key == Key::Enter {
                        // run filter
                        self.reload_results_table().await?;
                        self.focus = Focus::Table;
                        Ok(Consumed)
                    } else if key == Key::Esc {
                        self.focus = Focus::Table;
                        Ok(Consumed)
                    } else {
                        Ok(NotConsumed)
                    }
                }
            }
        };
    }

    async fn handle_messages(&mut self, messages: &Vec<Box<dyn AppMessage>>) -> Result<()> {
        for m in messages.iter() {
            handle_message!(m, DatabaseEvent,
                TableSelected(database,table) => {
                    self.reset();
                    self.update_table(database.clone(), table.clone()).await?;
                    let app_rhandle = self.app_state.read().await;
                    if let Some(src) = (*app_rhandle).pool_completion_src().await {
                        self.filter.update_completion_src(Box::new(src));
                    }
                }
            );
        }
        Ok(())
    }
}
