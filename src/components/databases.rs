use std::any::Any;
use std::collections::BTreeSet;
use std::convert::From;

use anyhow::Result;
use async_trait::async_trait;
use tui::{
    backend::Backend,
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Span, Spans},
    widgets::{Block, Borders},
};

use database_tree::{Database, DatabaseTree, DatabaseTreeItem, Table};

use crate::app::{AppMessage, GlobalMessageQueue, SharedPool};
use crate::components::command::{self, CommandInfo};
use crate::components::connections::ConnectionEvent;
use crate::config::{Connection, KeyConfig};
use crate::database::Pool;
use crate::event::Key;
use crate::ui::common_nav;
use crate::ui::scrolllist::draw_list_block;

use super::{
    Component, DatabaseFilterComponent, DrawableComponent, EventState,
    utils::scroll_vertical::VerticalScroll,
};

// ▸
const FOLDER_ICON_COLLAPSED: &str = "\u{25b8}";
// ▾
const FOLDER_ICON_EXPANDED: &str = "\u{25be}";

#[derive(PartialEq)]
pub enum Focus {
    Filter,
    Tree,
}

pub enum DatabaseEvent {
    TableSelected(Database, Table)
}

impl AppMessage for DatabaseEvent {
    fn as_any(&self) -> &(dyn Any + Send + Sync) {
        self
    }
}

pub struct DatabasesComponent {
    tree: DatabaseTree,
    filter: DatabaseFilterComponent,
    filterd_tree: Option<DatabaseTree>,
    scroll: VerticalScroll,
    focus: Focus,
    key_config: KeyConfig,
    shared_pool : SharedPool
}

impl DatabasesComponent {
    pub fn new(key_config: KeyConfig, shared_pool : SharedPool) -> Self {
        Self {
            tree: DatabaseTree::default(),
            filter: DatabaseFilterComponent::new(),
            filterd_tree: None,
            scroll: VerticalScroll::new(false, false),
            focus: Focus::Tree,
            key_config,
            shared_pool
        }
    }

    async fn update(&mut self, conn_opt: &Option<Connection>) -> Result<()> {
        // TODO: fix update block
        let mut databases: Vec<Database> = vec![];
        if let Some(pool_r_lock) = self.shared_pool.try_read().ok(){
            if let Some(pool) = pool_r_lock.as_ref() {
                if let Some(connection) = conn_opt {
                    databases = match &connection.database {
                        Some(database) => vec![Database::new(
                            database.clone(),
                            pool.get_tables(database.clone()).await?,
                        )],
                        None => pool.get_databases().await?,
                    };
                }
            }
        }

        self.tree = DatabaseTree::new(databases.as_slice(), &BTreeSet::new())?;
        self.filterd_tree = None;
        self.filter.reset();
        Ok(())
    }

    pub fn tree_focused(&self) -> bool {
        matches!(self.focus, Focus::Tree)
    }

    pub fn tree(&self) -> &DatabaseTree {
        self.filterd_tree.as_ref().unwrap_or(&self.tree)
    }

    fn tree_item_to_span(
        item: DatabaseTreeItem,
        selected: bool,
        width: u16,
        filter: Option<String>,
    ) -> Spans<'static> {
        let name = item.kind().name();
        let indent = item.info().indent();

        let indent_str = if indent == 0 {
            String::from("")
        } else {
            format!("{:w$}", " ", w = (indent as usize) * 2)
        };

        let arrow = if item.kind().is_database() || item.kind().is_schema() {
            if item.kind().is_database_collapsed() || item.kind().is_schema_collapsed() {
                FOLDER_ICON_COLLAPSED
            } else {
                FOLDER_ICON_EXPANDED
            }
        } else {
            // Naming self-explanatory constants is an anti-pattern, changing to literal value.
            ""
        };

        if let Some(filter) = filter {
            if item.kind().is_table() && name.contains(&filter) {
                let (first, rest) = &name.split_at(name.find(filter.as_str()).unwrap_or(0));
                let (middle, last) = &rest.split_at(filter.len().clamp(0, rest.len()));
                return Spans::from(vec![
                    Span::styled(
                        format!("{}{}{}", indent_str, arrow, first),
                        if selected {
                            Style::default().bg(Color::Blue)
                        } else {
                            Style::default()
                        },
                    ),
                    Span::styled(
                        middle.to_string(),
                        if selected {
                            Style::default().bg(Color::Blue).fg(Color::Blue)
                        } else {
                            Style::default().fg(Color::Blue)
                        },
                    ),
                    Span::styled(
                        format!("{:w$}", last.to_string(), w = width as usize),
                        if selected {
                            Style::default().bg(Color::Blue)
                        } else {
                            Style::default()
                        },
                    ),
                ]);
            }
        }

        Spans::from(Span::styled(
            format!("{}{}{:w$}", indent_str, arrow, name, w = width as usize),
            if selected {
                Style::default().bg(Color::Blue)
            } else {
                Style::default()
            },
        ))
    }

    fn draw_tree<B: Backend>(&self, f: &mut Frame<B>, area: Rect, focused: bool) -> Result<()> {
        f.render_widget(
            Block::default()
                .title("Databases")
                .borders(Borders::ALL)
                .style(if focused {
                    Style::default()
                } else {
                    Style::default().fg(Color::DarkGray)
                }),
            area,
        );

        let chunks = Layout::default()
            .vertical_margin(1)
            .horizontal_margin(1)
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Min(1)].as_ref())
            .split(area);

        self.filter
            .draw(f, chunks[0], matches!(self.focus, Focus::Filter))?;

        let tree_height = chunks[1].height as usize;
        let tree = if let Some(tree) = self.filterd_tree.as_ref() {
            tree
        } else {
            &self.tree
        };
        tree.visual_selection().map_or_else(
            || {
                self.scroll.reset();
            },
            |selection| {
                self.scroll
                    .update(selection.index, selection.count, tree_height);
            },
        );

        let items = tree
            .iterate(self.scroll.get_top(), tree_height)
            .map(|(item, selected)| {
                Self::tree_item_to_span(
                    item.clone(),
                    selected,
                    area.width,
                    if self.filter.input_str().is_empty() {
                        None
                    } else {
                        Some(self.filter.input_str())
                    },
                )
            });

        draw_list_block(f, chunks[1], Block::default().borders(Borders::NONE), items);
        self.scroll.draw(f, chunks[1]);

        Ok(())
    }
}

impl DrawableComponent for DatabasesComponent {
    fn draw<B: Backend>(&self, f: &mut Frame<B>, area: Rect, focused: bool) -> Result<()> {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(100)].as_ref())
            .split(area);

        self.draw_tree(f, chunks[0], focused)?;
        Ok(())
    }
}

#[async_trait]
impl Component for DatabasesComponent {
    fn commands(&self, out: &mut Vec<CommandInfo>) {
        out.push(CommandInfo::new(command::expand_collapse(&self.key_config)))
    }

    async fn event(&mut self, key: crate::event::Key, message_queue: &mut crate::app::GlobalMessageQueue) -> Result<EventState> {
        if key == self.key_config.filter && self.focus == Focus::Tree {
            self.focus = Focus::Filter;
            return Ok(EventState::Consumed);
        }

        if matches!(self.focus, Focus::Filter) {
            self.filterd_tree = if self.filter.input_str().is_empty() {
                None
            } else {
                Some(self.tree.filter(self.filter.input_str()))
            };
        }

        match key {
            Key::Enter if matches!(self.focus, Focus::Filter) => {
                self.focus = Focus::Tree;
                return Ok(EventState::Consumed);
            }
            key if matches!(self.focus, Focus::Filter) => {
                if self.filter.event(key, message_queue).await?.is_consumed() {
                    return Ok(EventState::Consumed);
                }
            }
            key => {
                if tree_nav(
                    if let Some(tree) = self.filterd_tree.as_mut() {
                        tree
                    } else {
                        &mut self.tree
                    },
                    key,
                    &self.key_config,
                ) {
                    return Ok(EventState::Consumed);
                }
            }
        }

        if  key == self.key_config.enter && matches!(self.focus, Focus::Tree) {
            if let Some((database, table)) = self.tree().selected_table() {
                message_queue.push(Box::new(DatabaseEvent::TableSelected(database, table)));
                return Ok(EventState::Consumed);
            }
        }

        Ok(EventState::NotConsumed)
    }

    async fn handle_messages(&mut self, messages: &Vec<Box<dyn AppMessage>>) -> Result<()> {
        for m in messages.iter() {
            if let Some(conn_event) = m.as_any().downcast_ref::<ConnectionEvent>() {
                match conn_event {
                    ConnectionEvent::ConnectionChanged(conn_opt) => {
                        self.reset();
                        self.update(conn_opt).await?;
                    }
                }
            }
        }
        Ok(())
    }
}

fn tree_nav(tree: &mut DatabaseTree, key: Key, key_config: &KeyConfig) -> bool {
    if let Some(common_nav) = common_nav(key, key_config) {
        tree.move_selection(common_nav)
    } else {
        false
    }
}

#[cfg(test)]
mod test {
    use database_tree::Table;

    use super::{Color, Database, DatabasesComponent, DatabaseTreeItem, Span, Spans, Style};

    #[test]
    fn test_tree_database_tree_item_to_span() {
        const WIDTH: u16 = 10;
        assert_eq!(
            DatabasesComponent::tree_item_to_span(
                DatabaseTreeItem::new_database(
                    &Database {
                        name: "foo".to_string(),
                        children: Vec::new(),
                    },
                    false,
                ),
                false,
                WIDTH,
                None,
            ),
            Spans::from(vec![Span::raw(format!(
                "\u{25b8}{:w$}",
                "foo",
                w = WIDTH as usize
            ))])
        );

        assert_eq!(
            DatabasesComponent::tree_item_to_span(
                DatabaseTreeItem::new_database(
                    &Database {
                        name: "foo".to_string(),
                        children: Vec::new(),
                    },
                    false,
                ),
                true,
                WIDTH,
                None,
            ),
            Spans::from(vec![Span::styled(
                format!("\u{25b8}{:w$}", "foo", w = WIDTH as usize),
                Style::default().bg(Color::Blue)
            )])
        );
    }

    #[test]
    fn test_tree_table_tree_item_to_span() {
        const WIDTH: u16 = 10;
        assert_eq!(
            DatabasesComponent::tree_item_to_span(
                DatabaseTreeItem::new_table(
                    &Database {
                        name: "foo".to_string(),
                        children: Vec::new(),
                    },
                    &Table {
                        name: "bar".to_string(),
                        create_time: None,
                        update_time: None,
                        engine: None,
                        schema: None
                    },
                ),
                false,
                WIDTH,
                None,
            ),
            Spans::from(vec![Span::raw(format!(
                "  {:w$}",
                "bar",
                w = WIDTH as usize
            ))])
        );

        assert_eq!(
            DatabasesComponent::tree_item_to_span(
                DatabaseTreeItem::new_table(
                    &Database {
                        name: "foo".to_string(),
                        children: Vec::new(),
                    },
                    &Table {
                        name: "bar".to_string(),
                        create_time: None,
                        update_time: None,
                        engine: None,
                        schema: None
                    },
                ),
                true,
                WIDTH,
                None,
            ),
            Spans::from(Span::styled(
                format!("  {:w$}", "bar", w = WIDTH as usize),
                Style::default().bg(Color::Blue),
            ))
        );
    }

    #[test]
    fn test_filterd_tree_item_to_span() {
        const WIDTH: u16 = 10;
        assert_eq!(
            DatabasesComponent::tree_item_to_span(
                DatabaseTreeItem::new_table(
                    &Database {
                        name: "foo".to_string(),
                        children: Vec::new(),
                    },
                    &Table {
                        name: "barbaz".to_string(),
                        create_time: None,
                        update_time: None,
                        engine: None,
                        schema: None
                    },
                ),
                false,
                WIDTH,
                Some("rb".to_string()),
            ),
            Spans::from(vec![
                Span::raw(format!("  {}", "ba")),
                Span::styled("rb", Style::default().fg(Color::Blue)),
                Span::raw(format!("{:w$}", "az", w = WIDTH as usize))
            ])
        );

        assert_eq!(
            DatabasesComponent::tree_item_to_span(
                DatabaseTreeItem::new_table(
                    &Database {
                        name: "foo".to_string(),
                        children: Vec::new(),
                    },
                    &Table {
                        name: "barbaz".to_string(),
                        create_time: None,
                        update_time: None,
                        engine: None,
                        schema: None
                    },
                ),
                true,
                WIDTH,
                Some("rb".to_string()),
            ),
            Spans::from(vec![
                Span::styled(format!("  {}", "ba"), Style::default().bg(Color::Blue)),
                Span::styled("rb", Style::default().bg(Color::Blue).fg(Color::Blue)),
                Span::styled(
                    format!("{:w$}", "az", w = WIDTH as usize),
                    Style::default().bg(Color::Blue)
                )
            ])
        );
    }
}
