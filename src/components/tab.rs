use std::any::Any;

use anyhow::Result;
use async_trait::async_trait;
use strum_macros::EnumIter;
use tui::layout::{Constraint, Direction, Layout};
use tui::widgets::canvas::Label;
use tui::widgets::Paragraph;
use tui::{
    backend::Backend,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Spans,
    widgets::{Block, Borders, Tabs},
    Frame,
};

use crate::app::{AppMessage, AppStateRef};
use crate::components::command::CommandInfo;
use crate::components::databases::DatabaseEvent;
use crate::components::EventState::{Consumed, NotConsumed};
use crate::components::{Drawable, PropertiesComponent, RecordTableComponent, SqlEditorComponent};
use crate::config::Config;
use crate::config::KeyConfig;
use crate::event::Key;
use crate::ui::textbox::TextBox;
use crate::{command, handle_message};

use super::{Component, DrawableComponent, EventState};

enum TabMessage {
    NewEditor,
    CloseCurrentEditor,
    RenameTab(usize, String),
}

impl AppMessage for TabMessage {
    fn as_any(&self) -> &(dyn Any + Send + Sync) {
        self
    }
}

#[derive(Debug, Clone, Copy, EnumIter)]
pub enum TabType {
    Records,
    Properties,
    Sql,
}

impl std::fmt::Display for TabType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub trait Tab<B: Backend>: Drawable<B> + Component + Send {
    fn tab_type(&self) -> TabType;
    fn tab_name(&self) -> String;
    fn update_name(&mut self, _name: String) {}
}

///TabToolbar - Toolbar for a TabPanel that contains a list of tab names and a selected tab index.
pub struct TabToolbar {
    pub selected_tab_index: usize,
    tab_names: Vec<String>,
    key_config: KeyConfig,
    is_renaming: bool,
    rename_box: TextBox,
}

impl TabToolbar {
    pub fn new(tab_names: Vec<String>, key_config: KeyConfig) -> Self {
        Self {
            selected_tab_index: 0,
            rename_box: TextBox::default()
                .with_placeholder("Editor name")
                .with_label("New name"),
            is_renaming: false,
            tab_names,
            key_config,
        }
    }

    fn add_tab(&mut self, tab_name: String) {
        self.tab_names.push(tab_name);
    }

    fn remove_tab(&mut self, index: usize) {
        self.tab_names.remove(index);

        if self.selected_tab_index >= self.tab_names.len() {
            self.selected_tab_index = self.tab_names.len() - 1;
        }
    }

    fn rename_tab_at(&mut self, index: usize, name: String) {
        if index > 0 && index < self.tab_names.len() {
            self.tab_names[index] = name;
        }
    }

    pub fn reset(&mut self) {
        self.selected_tab_index = 0;
    }
}

impl DrawableComponent for TabToolbar {
    fn draw<B: Backend>(&self, f: &mut Frame<B>, area: Rect, focused: bool) -> Result<()> {
        if self.is_renaming {
            self.rename_box.draw(f, area, true)?;
        } else {
            let titles = self
                .tab_names
                .iter()
                .enumerate()
                .map(|(i, name)| format!("{} [{}]", name, i + 1))
                .chain(std::iter::once("(Press 'a' for new editor)".to_string()))
                .map(Spans::from)
                .collect();
            let tabs = Tabs::new(titles)
                .block(Block::default().borders(Borders::ALL))
                .select(self.selected_tab_index)
                .style(if focused {
                    Style::default()
                } else {
                    Style::default().fg(Color::DarkGray)
                })
                .highlight_style(
                    Style::default()
                        .fg(Color::Reset)
                        .add_modifier(Modifier::UNDERLINED),
                );
            f.render_widget(tabs, area);
        }
        Ok(())
    }
}

#[async_trait]
impl Component for TabToolbar {
    fn commands(&self, commands: &mut Vec<CommandInfo>) {
        commands.push(command!("-- Tab bar --", "Open new editor [a]"));
        commands.push(command!("-- Tab bar --", "Close current editor [x,Del]"));
        commands.push(command!("-- Tab bar --", "Rename current editor [r]"));
        commands.push(command!("-- Tab bar --", "Cancel renaming [Esc]"));
    }

    async fn event(
        &mut self,
        key: crate::event::Key,
        message_queue: &mut crate::app::GlobalMessageQueue,
    ) -> Result<EventState> {
        if self.is_renaming {
            return match key {
                Key::Enter => {
                    let new_tab_name = self.rename_box.input_str();
                    self.is_renaming = false;
                    message_queue.push(Box::new(TabMessage::RenameTab(
                        self.selected_tab_index,
                        new_tab_name,
                    )));
                    Ok(Consumed)
                }
                Key::Esc => {
                    self.is_renaming = false;
                    Ok(Consumed)
                }
                _ => {
                    self.rename_box.event(key, message_queue).await?;
                    Ok(Consumed)
                }
            };
        } else if key == Key::Char('r') {
            self.rename_box.reset();
            let tab_name = &self.tab_names[self.selected_tab_index];
            self.rename_box.set_str(tab_name);
            self.is_renaming = true;
            return Ok(Consumed);
        }

        if Key::Char('a') == key {
            message_queue.push(Box::new(TabMessage::NewEditor));
            return Ok(Consumed);
        }

        if key == Key::Char('x') || key == Key::Delete {
            message_queue.push(Box::new(TabMessage::CloseCurrentEditor));
            return Ok(Consumed);
        }

        if !self.is_renaming && key == Key::Char('r') {
            self.rename_box.reset();
            self.is_renaming = true;
            return Ok(Consumed);
        }

        if let Key::Char(c) = key {
            if c.is_digit(10) {
                let tab_number = c.to_digit(10).unwrap() as usize;
                if tab_number > 0
                    && tab_number <= self.tab_names.len()
                    && !self.tab_names.is_empty()
                {
                    self.selected_tab_index = tab_number - 1;
                    return Ok(Consumed);
                }
                return Ok(NotConsumed);
            }
        }
        if key == self.key_config.focus_left && self.selected_tab_index > 0 {
            self.selected_tab_index -= 1;
            return Ok(Consumed);
        }

        if key == self.key_config.focus_right && self.selected_tab_index < self.tab_names.len() - 1
        {
            self.selected_tab_index += 1;
            return Ok(Consumed);
        }

        if key == Key::End {
            self.selected_tab_index = self.tab_names.len() - 1;
            return Ok(Consumed);
        }

        if key == Key::Home {
            self.selected_tab_index = 0;
            return Ok(Consumed);
        }

        Ok(NotConsumed)
    }
}

enum Focus {
    Toolbar,
    Content,
}

pub struct TabPanel<B: Backend> {
    config: Config,
    toolbar: TabToolbar,
    tab_components: Vec<Box<dyn Tab<B>>>,
    focus: Focus,
    app_state: AppStateRef,
}

impl<B: Backend> Drawable<B> for TabPanel<B> {
    fn draw(&mut self, f: &mut Frame<B>, area: Rect, focused: bool) -> Result<()> {
        let block = tui::widgets::Block::default()
            .borders(Borders::ALL)
            .style(if focused {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::DarkGray)
            });
        f.render_widget(block, area);

        let tab_panel_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Length(5)].as_ref())
            .split(area);

        self.toolbar.draw(
            f,
            tab_panel_chunks[0],
            focused && matches!(self.focus, Focus::Toolbar),
        )?;
        if let Some(tab_content) = self.tab_components.get_mut(self.toolbar.selected_tab_index) {
            tab_content.draw(
                f,
                tab_panel_chunks[1],
                focused && matches!(self.focus, Focus::Content),
            )?;
        }
        Ok(())
    }
}

#[async_trait]
impl<B: Backend> Component for TabPanel<B> {
    fn commands(&self, _out: &mut Vec<CommandInfo>) {
        self.toolbar.commands(_out);
        for tab in self.tab_components.iter() {
            tab.commands(_out);
        }
    }
    async fn event(
        &mut self,
        key: crate::event::Key,
        message_queue: &mut crate::app::GlobalMessageQueue,
    ) -> Result<EventState> {
        match self.focus {
            Focus::Toolbar => {
                if self.toolbar.event(key, message_queue).await?.is_consumed() {
                    return Ok(EventState::Consumed);
                }
            }
            Focus::Content => {
                if let Some(content) = self.tab_components.get_mut(self.toolbar.selected_tab_index)
                {
                    if content.event(key, message_queue).await?.is_consumed() {
                        return Ok(EventState::Consumed);
                    }
                }
            }
        }

        if self.change_focus(key)?.is_consumed() {
            return Ok(Consumed);
        }

        Ok(EventState::NotConsumed)
    }

    async fn handle_messages(&mut self, messages: &Vec<Box<dyn AppMessage>>) -> Result<()> {
        use futures::future::join_all;
        // use crate::components::handle_message;
        for m in messages.iter() {
            handle_message!(m,DatabaseEvent,
                DatabaseEvent::TableSelected(_,_) => {
                    self.toolbar.selected_tab_index = 0;
                    self.focus = Focus::Content
                }
            );

            handle_message!(m, TabMessage,
                TabMessage::NewEditor => {
                    let num = self.tab_components.len() - 1;
                    let tab_name = format!("Sql Editor {}", num);
                    let new_editor = SqlEditorComponent::new(self.config.key_config.clone(), self.app_state.clone(), Some(tab_name.clone())).await;
                    self.tab_components.push(Box::new(new_editor));
                    self.toolbar.add_tab(tab_name);
                },TabMessage::CloseCurrentEditor => {
                    self.close_selected_editor();
                }, TabMessage::RenameTab(index, new_name) => {
                    if let Some(tab) = self.tab_components.get_mut(index.clone()) {
                        tab.update_name(new_name.clone());
                        self.toolbar.rename_tab_at(index.clone(), tab.tab_name());
                    }
                }
            );
        }
        // pass to children
        return join_all(
            self.tab_components
                .iter_mut()
                .map(|t| t.handle_messages(messages)),
        )
        .await
        .drain(0..)
        .reduce(Result::and)
        .unwrap();
    }

    fn reset(&mut self) {
        self.toolbar.selected_tab_index = 0;
        self.tab_components.iter_mut().for_each(|t| t.reset());
    }
}

impl<B: Backend> TabPanel<B> {
    pub async fn new(config: Config, app_state: AppStateRef) -> TabPanel<B> {
        let tab_components: Vec<Box<dyn Tab<B>>> = vec![
            Box::new(RecordTableComponent::new(
                config.key_config.clone(),
                app_state.clone(),
            )),
            Box::new(PropertiesComponent::new(
                config.key_config.clone(),
                app_state.clone(),
            )),
        ];
        return TabPanel {
            config: config.clone(),
            toolbar: TabToolbar::new(
                tab_components.iter().map(|t| t.tab_name()).collect(),
                config.key_config,
            ),
            tab_components,
            focus: Focus::Toolbar,
            app_state,
        };
    }

    fn close_selected_editor(&mut self) {
        let index = self.toolbar.selected_tab_index;
        if let Some(tab) = self.tab_components.get(index) {
            match tab.tab_type() {
                TabType::Records | TabType::Properties => return (),
                _ => (),
            }
        } else {
            return;
        }

        self.tab_components.remove(index);
        self.toolbar.remove_tab(index);
    }

    fn change_focus(&mut self, key: Key) -> Result<EventState> {
        match self.focus {
            Focus::Toolbar => {
                if self.config.key_config.focus_down == key {
                    self.focus = Focus::Content;
                    return Ok(Consumed);
                }
            }
            Focus::Content => {
                if self.config.key_config.focus_above == key {
                    self.focus = Focus::Toolbar;
                    return Ok(Consumed);
                }
            }
        }
        return Ok(NotConsumed);
    }
}
