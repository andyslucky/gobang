use std::any::Any;
use anyhow::Result;
use futures::StreamExt;
use strum_macros::EnumIter;
use tui::{
    backend::Backend,
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Spans,
    widgets::{Block, Borders, Tabs},
};
use tui::layout::{Constraint, Direction, Layout};

use crate::components::{Drawable, PropertiesComponent, RecordTableComponent, SqlEditorComponent};
use crate::components::command::{self, CommandInfo};
use crate::components::databases::{DatabaseEvent, DatabaseMessageObserver};
use crate::components::EventState::{Consumed, NotConsumed};
use crate::config::Config;
use crate::config::KeyConfig;
use crate::database::Pool;
use crate::event::Key;

use super::{Component, DrawableComponent, EventState};

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

pub trait Tab<B : Backend> : Drawable<B> + Component + DatabaseMessageObserver{
    fn tab_type(&self) -> TabType;
    fn tab_name(&self) -> String;
}

/// Toolbar containing the name of each tab 
pub struct TabToolbar {
    pub selected_tab_index : usize,
    tab_names: Vec<String>,
    key_config: KeyConfig,
}

impl TabToolbar {
    pub fn new(tab_names : Vec<String>, key_config: KeyConfig) -> Self {
        Self {
            selected_tab_index: 0, tab_names, key_config
        }
    }

    pub fn reset(&mut self) {
        self.selected_tab_index = 0;
    }
}

/// Draws tab panel on screen, 
impl DrawableComponent for TabToolbar {
    fn draw<B: Backend>(&self, f: &mut Frame<B>, area: Rect, focused: bool) -> Result<()> {
        let titles =
            self.tab_names.iter()
                .enumerate()
                .map(|(i, name)| Spans::from(format!("{} [{}]", name, i + 1)))
                .collect();
        let tabs = Tabs::new(titles)
            .block(Block::default().borders(Borders::ALL))
            .select(self.selected_tab_index)
            .style(if focused { Style::default()} else {Style::default().fg(Color::DarkGray)})
            .highlight_style(
                Style::default()
                    .fg(Color::Reset)
                    .add_modifier(Modifier::UNDERLINED),
            );
        f.render_widget(tabs, area);
        Ok(())
    }
}

impl Component for TabToolbar {
    fn commands(&self, _out: &mut Vec<CommandInfo>) {}

    fn event(&mut self, key: Key) -> Result<EventState> {
        if let Key::Char(c) = key {
            if c.is_digit(10) {
                let tab_number = c.to_digit(10).unwrap() as usize;
                if tab_number > 0 {
                    self.selected_tab_index = tab_number - 1;
                    return Ok(EventState::Consumed);
                }
                return Ok(EventState::Consumed);
            }
        }
        Ok(EventState::NotConsumed)
    }
}

enum Focus {
    Toolbar,
    Content
}

pub struct TabPanel<B : Backend> {
    config : Config,
    toolbar : TabToolbar,
    tab_components : Vec<Box<dyn Tab<B>>>,
    focus : Focus
}



impl<B: Backend> Drawable<B> for TabPanel<B> {
    fn draw(&mut self, f: &mut Frame<B>, area: Rect, focused: bool) -> Result<()> {
        let tab_panel_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Length(5)].as_ref())
            .split(area);

        self.toolbar.draw(f, tab_panel_chunks[0], focused && matches!(self.focus, Focus::Toolbar))?;
        if let Some(tab_content) = self.tab_components.get_mut(self.toolbar.selected_tab_index) {
            tab_content.draw(f,tab_panel_chunks[1], focused && matches!(self.focus, Focus::Content))?;
        }

        Ok(())
    }
}

impl<B : Backend> Component for TabPanel<B> {
    fn commands(&self, out: &mut Vec<CommandInfo>) {

    }

    fn event(&mut self, key: Key) -> Result<EventState> {

        match self.focus {
            Focus::Toolbar => {
                if self.toolbar.event(key)?.is_consumed() {
                    return Ok(EventState::Consumed)
                }
            },
            Focus::Content => {
                if let Some(content) = self.tab_components.get_mut(self.toolbar.selected_tab_index) {
                    if content.event(key)?.is_consumed() {
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

    fn reset(&mut self) {
        self.toolbar.selected_tab_index = 0;
        self.tab_components.iter_mut().for_each(|t| t.reset());
    }

}

impl<B : Backend> DatabaseMessageObserver for TabPanel<B> {
    fn handle_message(&mut self, message: &DatabaseEvent) -> Result<()> {
        match message {
            DatabaseEvent::TableSelected(_, _) => {
                self.toolbar.selected_tab_index = 0;
                self.tab_components.iter_mut().for_each(|t| t.handle_message(message).unwrap())
            }
        }
        Ok(())
    }
}


impl<B : Backend> TabPanel<B>{
    pub fn new(config : Config) -> TabPanel<B> {
        let tab_components : Vec<Box<dyn Tab<B>>> = vec![
            Box::new(RecordTableComponent::new(config.key_config.clone())),
            Box::new(PropertiesComponent::new(config.key_config.clone())),
            Box::new(SqlEditorComponent::new(config.key_config.clone()))
        ];
        return TabPanel {
            config: config.clone(),
            toolbar : TabToolbar::new(tab_components.iter().map(|t| t.tab_name()).collect(), config.key_config),
            tab_components,
            focus: Focus::Toolbar
        };
    }

    fn change_focus(&mut self, key : Key) -> Result<EventState> {
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
        return Ok(NotConsumed)
    }

    // fn handle(&mut self, message: &DatabaseEvent) -> Result<()> {
    //     use std::any::Any;
    //     self.toolbar.selected_tab_index = 0;
    //     self.tab_components.iter_mut().map(|c| c as Box<dyn Any>).for_each(|c|{
    //         let mut c_any : &mut Box<dyn Any> = c;
    //         let a = c_any.downcast_mut::<dyn MessageHandler<DatabaseEvent>>();
    //         if let Some(h) = c_any.downcast_ref::<dyn MessageHandler<DatabaseEvent>>() {
    //             h.handle(message);
    //         }
    //     });
    //     Ok(())
    // }
}



