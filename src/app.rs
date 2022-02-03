use std::any::Any;
use std::sync::Arc;

use tokio::sync::RwLock;
use tui::{
    backend::Backend,
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
};
use tui::style::{Color, Style};
use tui::widgets::Block;

use crate::{components::{
    command, ConnectionsComponent, DatabasesComponent, ErrorComponent, HelpComponent
}, config::Config, handle_message};
use crate::components::{
    CommandInfo, Component as _, Drawable, DrawableComponent as _, EventState
};
use crate::components::connections::ConnectionEvent;
use crate::components::databases::DatabaseEvent;
use crate::components::tab::TabPanel;
use crate::config::Connection;
use crate::database::{MySqlPool, Pool, PostgresPool, SqlitePool};
use crate::event::Key;

pub type SharedPool = Arc<RwLock<Option<Box<dyn Pool>>>>;

/// Dynamic trait representing a message/event. Messages may be added to the global event queue during
/// by any component's event handler. The global message queue will be processed at the end of each key event
/// and at the end of each tick.
pub trait AppMessage : Send + Sync{
    fn as_any(&self) -> &(dyn Any + Send + Sync);
}


/// Global event queue. Stores queued events until the
pub struct GlobalMessageQueue {
    event_queue : Vec<Box<dyn AppMessage>>
}



impl GlobalMessageQueue {
    fn drain(&mut self) -> Vec<Box<dyn AppMessage>> {
        if self.event_queue.is_empty() {return vec![];}
        return self.event_queue.drain(0..).collect();
    }

    pub fn push(&mut self, message : Box<dyn AppMessage>) {
        self.event_queue.push(message);
    }
}

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
    pool: SharedPool,
    left_main_chunk_percentage: u16,
    message_queue : GlobalMessageQueue,
    pub config: Config,
    pub error: ErrorComponent,
}

impl<B : Backend> App<B> {
    pub fn new(config: Config) -> App<B> {
        let config_clone = config.clone();
        let share_pool = Arc::new(RwLock::new(None));
         App {
            config: config.clone(),
            connections: ConnectionsComponent::new(config.key_config.clone(), config.conn),
            tab_panel: TabPanel::new(config_clone,share_pool.clone()),
            help: HelpComponent::new(config.key_config.clone()),
            databases: DatabasesComponent::new(config.key_config.clone(), share_pool.clone()),
            error: ErrorComponent::new(config.key_config),
            focus: Focus::ConnectionList,
            pool: share_pool.clone(),
            message_queue: GlobalMessageQueue{event_queue: vec![]},
            left_main_chunk_percentage: 15,
        }
    }

    pub fn draw(&mut self, f: &mut Frame<'_, B>) -> anyhow::Result<()> {
        let main_block = Block::default().style(Style::default().bg(Color::Rgb(0x21,0x2a,0x31)));
        f.render_widget(main_block,f.size());
        if let Focus::ConnectionList = self.focus {
            self.connections.draw(
                f,
                Layout::default()
                    .constraints([Constraint::Percentage(100)])
                    .split(f.size())[0],
                false,
            )?;
        } else {

            let main_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(self.left_main_chunk_percentage),
                    Constraint::Percentage((100_u16).saturating_sub(self.left_main_chunk_percentage)),
                ])
                .split(f.size());
            let sidebar_chunk = main_chunks[0];
            let content_chunk = main_chunks[1];
            if sidebar_chunk.width > 0 {
                self.databases.draw(f, sidebar_chunk, matches!(self.focus, Focus::DatabaseList))?;
            }
            if content_chunk.width > 0 {
                self.tab_panel.draw(f, content_chunk, matches!(self.focus, Focus::TabPanel))?;
            }
        }
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

    async fn get_pool_from_conn(&mut self, conn: &Connection) -> anyhow::Result<Box<dyn Pool>> {
        return if conn.is_mysql() {
            Ok(Box::new(
                MySqlPool::new(conn.database_url()?.as_str()).await?,
            ))
        } else if conn.is_postgres() {
            Ok(Box::new(
                PostgresPool::new(conn.database_url()?.as_str()).await?,
            ))
        } else {
            Ok(Box::new(
                SqlitePool::new(conn.database_url()?.as_str()).await?,
            ))
        }
    }

    pub async fn event(&mut self, key: Key) -> anyhow::Result<EventState> {
        self.update_commands();
        let mut result : anyhow::Result<EventState> = Ok(EventState::NotConsumed);
            // send the event to all children, if it is handled then return
        if self.components_event(key).await?.is_consumed() {
            result = Ok(EventState::Consumed);
        } else if self.move_focus(key)?.is_consumed() {
            result = Ok(EventState::Consumed)
        }

        self.dispatch_messages().await?;
        return result;
    }
    async fn on_conn_changed(&mut self, conn: &Connection) {
        if let Some(new_pool) = self.get_pool_from_conn(conn).await.ok() {
            let mut pool_w_lock = self.pool.write().await;
            if let Some(current_pool) = pool_w_lock.as_ref() {
                current_pool.close().await;
            }
            (*pool_w_lock) = Some(new_pool);
        }
        self.focus = Focus::DatabaseList;
    }

    async fn handle_messages(&mut self, messages : &mut Vec<Box<dyn AppMessage>>) -> anyhow::Result<()>{

        for m in messages.iter() {
            handle_message!(m, ConnectionEvent,
                ConnectionEvent::ConnectionChanged(conn_opt) => {
                    if let Some(conn) = conn_opt {
                        self.on_conn_changed(conn).await;
                    }
                }
            );
            handle_message!(m, DatabaseEvent,
                DatabaseEvent::TableSelected(_,_) => {self.focus = Focus::TabPanel;}
            )
        }
        Ok(())
    }

    /// Drains the global message queue and passes messages to all components simultaneously.
    async fn dispatch_messages(&mut self) -> anyhow::Result<()> {
        let mut messages = self.message_queue.drain();

        if !messages.is_empty() {
            // dispatch messages on app first.
            self.handle_messages(&mut messages).await?;
            // Send messages to each child component
            return futures::future::join_all(vec![
                self.databases.handle_messages(&messages),
                self.tab_panel.handle_messages(&messages),
                self.connections.handle_messages(&messages)
            ]).await.drain(0..).reduce(Result::and).unwrap();
        }
        Ok(())
    }

    async fn components_event(&mut self, key: Key) -> anyhow::Result<EventState> {
        if self.error.event(key, &mut self.message_queue).await?.is_consumed() {
            return Ok(EventState::Consumed);
        }

        if !matches!(self.focus, Focus::ConnectionList) && self.help.event(key, &mut self.message_queue).await?.is_consumed() {
            return Ok(EventState::Consumed);
        }

        match self.focus {
            Focus::ConnectionList => {
                if self.connections.event(key, &mut self.message_queue).await?.is_consumed() {
                    return Ok(EventState::Consumed);
                }
            }
            Focus::DatabaseList => {
                if self.databases.event(key, &mut self.message_queue).await?.is_consumed() {
                    return Ok(EventState::Consumed);
                }
            }
            Focus::TabPanel => {
                if self.tab_panel.event(key, &mut self.message_queue).await?.is_consumed() {
                    return Ok(EventState::Consumed)
                }
                // TODO: Reimplement this section in the records tab
                // match self.tab.selected_tab {
                //     TabType::Records => {
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
                self.left_main_chunk_percentage.saturating_sub(5).max(0);
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
    #[allow(unused_imports)]
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
