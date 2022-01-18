use crate::clipboard::copy_to_clipboard;
use crate::components::{
    CommandInfo, Component as _, DrawableComponent as _, EventState, Drawable
};
use crate::database::{MySqlPool, Pool, PostgresPool, SqlitePool, RECORDS_LIMIT_PER_PAGE};
use crate::event::Key;
use crate::{
    components::tab::TabType,
    components::{
        command, ConnectionsComponent, DatabasesComponent, ErrorComponent, HelpComponent,
        PropertiesComponent, RecordTableComponent, SqlEditorComponent,TabToolbar
    },
    config::Config,
};
use crate::components::tab::TabPanel;
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    Frame,
};
use crate::components::databases::{DatabaseEvent, DatabaseMessageObserver};

pub enum Focus {
    DatabaseList,
    TabPanel,
    ConnectionList,
}
pub struct App<B : Backend> {
    focus: Focus,
    tab_panel : TabPanel<B>,
    help: HelpComponent,
    databases: DatabasesComponent,
    connections: ConnectionsComponent,
    pool: Option<Box<dyn Pool>>,
    left_main_chunk_percentage: u16,
    pub config: Config,
    pub error: ErrorComponent,
}

impl<B : Backend> App<B> {
    pub fn new(config: Config) -> App<B> {
        let config_clone = config.clone();
         App {
            config: config.clone(),
            connections: ConnectionsComponent::new(config.key_config.clone(), config.conn),
            tab_panel: TabPanel::new(config_clone),
            help: HelpComponent::new(config.key_config.clone()),
            databases: DatabasesComponent::new(config.key_config.clone()),
            error: ErrorComponent::new(config.key_config),
            focus: Focus::ConnectionList,
            pool: None,
            left_main_chunk_percentage: 15,
        }
    }

    pub fn draw(&mut self, f: &mut Frame<'_, B>) -> anyhow::Result<()> {
        if let Focus::ConnectionList = self.focus {
            self.connections.draw(
                f,
                Layout::default()
                    .constraints([Constraint::Percentage(100)])
                    .split(f.size())[0],
                false,
            )?;
            self.error.draw(f, Rect::default(), false)?;
            self.help.draw(f, Rect::default(), false)?;
            return Ok(());
        }

        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(self.left_main_chunk_percentage),
                Constraint::Percentage((100_u16).saturating_sub(self.left_main_chunk_percentage)),
            ])
            .split(f.size());
        let sidebar_chunk = main_chunks[0];
        let content_chunk = main_chunks[1];

        self.databases
            .draw(f, sidebar_chunk, matches!(self.focus, Focus::DatabaseList))?;

        self.tab_panel
            .draw(f, content_chunk, matches!(self.focus, Focus::TabPanel))?;

        self.error.draw(f, Rect::default(), false)?;
        self.help.draw(f, Rect::default(), false)?;
        Ok(())
    }

    fn update_commands(&mut self) {
        self.help.set_cmds(self.commands());
    }

    fn commands(&self) -> Vec<CommandInfo> {
        let mut res = vec![
            CommandInfo::new(command::filter(&self.config.key_config)),
            CommandInfo::new(command::help(&self.config.key_config)),
            CommandInfo::new(command::toggle_tabs(&self.config.key_config)),
            CommandInfo::new(command::scroll(&self.config.key_config)),
            CommandInfo::new(command::scroll_to_top_bottom(&self.config.key_config)),
            CommandInfo::new(command::scroll_up_down_multiple_lines(
                &self.config.key_config,
            )),
            CommandInfo::new(command::move_focus(&self.config.key_config)),
            CommandInfo::new(command::extend_or_shorten_widget_width(
                &self.config.key_config,
            )),
        ];

        self.databases.commands(&mut res);
        self.tab_panel.commands(&mut res);

        res
    }

    async fn update_databases(&mut self) -> anyhow::Result<()> {
        if let Some(conn) = self.connections.selected_connection() {
            if let Some(pool) = self.pool.as_ref() {
                pool.close().await;
            }
            self.pool = if conn.is_mysql() {
                Some(Box::new(
                    MySqlPool::new(conn.database_url()?.as_str()).await?,
                ))
            } else if conn.is_postgres() {
                Some(Box::new(
                    PostgresPool::new(conn.database_url()?.as_str()).await?,
                ))
            } else {
                Some(Box::new(
                    SqlitePool::new(conn.database_url()?.as_str()).await?,
                ))
            };
            self.databases
                .update(conn, self.pool.as_ref().unwrap())
                .await?;
            self.focus = Focus::DatabaseList;
            // TODO: Reimplement reset
            // self.record_table.reset();
            // self.tab.reset();
        }
        Ok(())
    }

    async fn update_record_table(&mut self) -> anyhow::Result<()> {
        // if let Some((database, table)) = self.databases.tree().selected_table() {
        //     let (headers, records) = self
        //         .pool
        //         .as_ref()
        //         .unwrap()
        //         .get_records(
        //             &database,
        //             &table,
        //             0,
        //             if self.record_table.filter.input_str().is_empty() {
        //                 None
        //             } else {
        //                 Some(self.record_table.filter.input_str())
        //             },
        //         )
        //         .await?;
        //     self.record_table
        //         .update(records, headers, database.clone(), table.clone());
        // }
        Ok(())
    }

    pub async fn event(&mut self, key: Key) -> anyhow::Result<EventState> {
        self.update_commands();

        // send the event to all children, if it is handled then return
        if self.components_event(key).await?.is_consumed() {
            return Ok(EventState::Consumed);
        };

        if self.move_focus(key)?.is_consumed() {
            return Ok(EventState::Consumed);
        };
        Ok(EventState::NotConsumed)
    }

    async fn components_event(&mut self, key: Key) -> anyhow::Result<EventState> {
        if self.error.event(key)?.is_consumed() {
            return Ok(EventState::Consumed);
        }

        if !matches!(self.focus, Focus::ConnectionList) && self.help.event(key)?.is_consumed() {
            return Ok(EventState::Consumed);
        }

        match self.focus {
            Focus::ConnectionList => {
                if self.connections.event(key)?.is_consumed() {
                    return Ok(EventState::Consumed);
                }

                if key == self.config.key_config.enter {
                    self.update_databases().await?;
                    return Ok(EventState::Consumed);
                }
            }
            Focus::DatabaseList => {
                if self.databases.event(key)?.is_consumed() {
                    return Ok(EventState::Consumed);
                }

                 if key == self.config.key_config.enter && self.databases.tree_focused() {
                     if let Some((database, table)) = self.databases.tree().selected_table() {
                         self.tab_panel.handle_message(&DatabaseEvent::TableSelected(database,table))?;
                     }

                 }
                // if key == self.config.key_config.enter && self.databases.tree_focused() {
                //     if let Some((database, table)) = self.databases.tree().selected_table() {
                //         self.record_table.reset();
                //         let (headers, records) = self
                //             .pool
                //             .as_ref()
                //             .unwrap()
                //             .get_records(&database, &table, 0, None)
                //             .await?;
                //         self.record_table
                //             .update(records, headers, database.clone(), table.clone());
                //         self.properties
                //             .update(database.clone(), table.clone(), self.pool.as_ref().unwrap())
                //             .await?;
                //         self.focus = Focus::TabPanel;
                //     }
                //     return Ok(EventState::Consumed);
                // }
            }
            Focus::TabPanel => {
                if self.tab_panel.event(key)?.is_consumed() {
                    return Ok(EventState::Consumed)
                }
                // match self.tab.selected_tab {
                //     TabType::Records => {
                //         if self.record_table.event(key)?.is_consumed() {
                //             return Ok(EventState::Consumed);
                //         };
                //
                //         if key == self.config.key_config.copy {
                //             if let Some(text) = self.record_table.table.selected_cells() {
                //                 copy_to_clipboard(text.as_str())?
                //             }
                //         }
                //
                //         if key == self.config.key_config.enter && self.record_table.filter_focused()
                //         {
                //             self.record_table.focus = crate::components::record_table::Focus::Table;
                //             self.update_record_table().await?;
                //         }
                //
                //         if self.record_table.table.eod {
                //             return Ok(EventState::Consumed);
                //         }
                //
                //         if let Some(index) = self.record_table.table.selected_row.selected() {
                //             if index.saturating_add(1) % RECORDS_LIMIT_PER_PAGE as usize == 0 {
                //                 if let Some((database, table)) =
                //                     self.databases.tree().selected_table()
                //                 {
                //                     let (_, records) = self
                //                         .pool
                //                         .as_ref()
                //                         .unwrap()
                //                         .get_records(
                //                             &database,
                //                             &table,
                //                             index as u16,
                //                             if self.record_table.filter.input_str().is_empty() {
                //                                 None
                //                             } else {
                //                                 Some(self.record_table.filter.input_str())
                //                             },
                //                         )
                //                         .await?;
                //                     if !records.is_empty() {
                //                         self.record_table.table.rows.extend(records);
                //                     } else {
                //                         self.record_table.table.end()
                //                     }
                //                 }
                //             }
                //         };
                //     }
                //     TabType::Sql => {
                //         if self.sql_editor.event(key)?.is_consumed()
                //             || self
                //                 .sql_editor
                //                 .async_event(key, self.pool.as_ref().unwrap())
                //                 .await?
                //                 .is_consumed()
                //         {
                //             return Ok(EventState::Consumed);
                //         };
                //     }
                //     TabType::Properties => {
                //         if self.properties.event(key)?.is_consumed() {
                //             return Ok(EventState::Consumed);
                //         };
                //     }
                // };
            }
        }

        if self.extend_or_shorten_widget_width(key)?.is_consumed() {
            return Ok(EventState::Consumed);
        };

        Ok(EventState::NotConsumed)
    }

    fn extend_or_shorten_widget_width(&mut self, key: Key) -> anyhow::Result<EventState> {
        if key
            == self
                .config
                .key_config
                .extend_or_shorten_widget_width_to_left
        {
            self.left_main_chunk_percentage =
                self.left_main_chunk_percentage.saturating_sub(5).max(15);
            return Ok(EventState::Consumed);
        } else if key
            == self
                .config
                .key_config
                .extend_or_shorten_widget_width_to_right
        {
            self.left_main_chunk_percentage = (self.left_main_chunk_percentage + 5).min(70);
            return Ok(EventState::Consumed);
        }
        Ok(EventState::NotConsumed)
    }

    fn move_focus(&mut self, key: Key) -> anyhow::Result<EventState> {
        if key == self.config.key_config.focus_connections {
            self.focus = Focus::ConnectionList;
            return Ok(EventState::Consumed);
        }
        // if self.tab.event(key)?.is_consumed() {
        //     return Ok(EventState::Consumed);
        // }
        match self.focus {
            Focus::ConnectionList => {
                if key == self.config.key_config.enter {
                    self.focus = Focus::DatabaseList;
                    return Ok(EventState::Consumed);
                }
            }
            Focus::DatabaseList => {
                if key == self.config.key_config.focus_right && self.databases.tree_focused() {
                    self.focus = Focus::TabPanel;
                    return Ok(EventState::Consumed);
                }
            }
            Focus::TabPanel => {
                if key == self.config.key_config.focus_left {
                    self.focus = Focus::DatabaseList;
                    return Ok(EventState::Consumed);
                }
            }
        }
        Ok(EventState::NotConsumed)
    }
}

#[cfg(test)]
mod test {
    use super::{App, Config, EventState, Key};

    #[test]
    fn test_extend_or_shorten_widget_width() {
        // let mut app = App::new(Config::default());
        // assert_eq!(
        //     app.extend_or_shorten_widget_width(Key::Char('>')).unwrap(),
        //     EventState::Consumed
        // );
        // assert_eq!(app.left_main_chunk_percentage, 20);

        // app.left_main_chunk_percentage = 70;
        // assert_eq!(
        //     app.extend_or_shorten_widget_width(Key::Char('>')).unwrap(),
        //     EventState::Consumed
        // );
        // assert_eq!(app.left_main_chunk_percentage, 70);

        // assert_eq!(
        //     app.extend_or_shorten_widget_width(Key::Char('<')).unwrap(),
        //     EventState::Consumed
        // );
        // assert_eq!(app.left_main_chunk_percentage, 65);

        // app.left_main_chunk_percentage = 15;
        // assert_eq!(
        //     app.extend_or_shorten_widget_width(Key::Char('<')).unwrap(),
        //     EventState::Consumed
        // );
        // assert_eq!(app.left_main_chunk_percentage, 15);
    }
}
