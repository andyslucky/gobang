use anyhow::Result;
use async_trait::async_trait;
use log::{debug, info};
use tui::{
    backend::Backend,
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, Paragraph},
};
use tui::layout::{Constraint, Direction, Layout};
use unicode_width::UnicodeWidthStr;

use database_tree::Table;

use crate::components::{Drawable, DrawableComponent};
use crate::components::command::CommandInfo;
use crate::components::EventState::{Consumed, NotConsumed};
use crate::config::KeyConfig;
use crate::event::Key;
use crate::ui::ComponentStyles;
use crate::ui::textbox::TextBox;

use super::{
    CompletionComponent, Component, compute_character_width, EventState, MovableComponent
};

pub struct TableFilterComponent {
    key_config: KeyConfig,
    pub table: Option<Table>,
    text_box : TextBox,
    completion: CompletionComponent,
}

impl TableFilterComponent {
    pub fn new(key_config: KeyConfig) -> Self {
        Self {
            key_config: key_config.clone(),
            table: None,
            text_box : TextBox::default()
                .with_placeholder("Enter SQL expression to filter records")
                .with_styles(ComponentStyles{borders: Some(Borders::BOTTOM)}),
            completion: CompletionComponent::new(key_config, "", false),
        }
    }

    pub fn set_table(&mut self, table : Table) {
        self.text_box.set_label(table.name.clone());
        self.table = Some(table);
    }

    pub fn input_str(&self) -> String {
        self.text_box.input_str()
    }

    pub fn reset(&mut self) {
        self.table = None;
        self.text_box.reset();
    }

    fn complete(&mut self) -> anyhow::Result<EventState> {

        Ok(EventState::NotConsumed)
    }
}

impl<B : Backend> Drawable<B> for TableFilterComponent {
    fn draw(&mut self, f: &mut Frame<B>, area: Rect, focused: bool) -> Result<()> {
        self.text_box.draw(f, area, focused)?;
        let (cursor_x,cursor_y) = self.text_box.cursor_position(&area);
        self.completion.draw(f,area,false,cursor_x,cursor_y + 1)?;
        Ok(())
    }
}

#[async_trait]
impl Component for TableFilterComponent {
    fn commands(&self, _out: &mut Vec<CommandInfo>) {}

    async fn event(&mut self, key: crate::event::Key, message_queue: &mut crate::app::GlobalMessageQueue) -> Result<EventState> {
        if self.text_box.event(key, message_queue).await?.is_consumed() {
            if let Some(last_w) = self.text_box.last_word_part() {
                debug!("Last word part '{}'", last_w);
                self.completion.update(last_w);
            }
            return Ok(Consumed);
        }
        if self.completion.event(key, message_queue).await?.is_consumed() {
            return Ok(Consumed);
        }

        if key == Key::Enter && self.completion.is_visible(){
            if let Some(candidate) = self.completion.selected_candidate() {
                self.text_box.replace_last_word_part(candidate);
                self.completion.update("");
                return Ok(Consumed);
            }
        }

        return Ok(NotConsumed);
    }
}

#[cfg(test)]
mod test {
    use super::{KeyConfig, TableFilterComponent};

    #[test]
    fn test_complete() {
        // let mut filter = TableFilterComponent::new(KeyConfig::default());
        // filter.input_idx = 2;
        // filter.input = vec!['a', 'n', ' ', 'c', 'd', 'e', 'f', 'g'];
        // filter.completion.update("an");
        // assert!(filter.complete().is_ok());
        // assert_eq!(
        //     filter.input,
        //     vec!['A', 'N', 'D', ' ', 'c', 'd', 'e', 'f', 'g']
        // );
    }

    #[test]
    fn test_complete_end() {
        // let mut filter = TableFilterComponent::new(KeyConfig::default());
        // filter.input_idx = 9;
        // filter.input = vec!['a', 'b', ' ', 'c', 'd', 'e', 'f', ' ', 'i'];
        // filter.completion.update('i');
        // assert!(filter.complete().is_ok());
        // assert_eq!(
        //     filter.input,
        //     vec!['a', 'b', ' ', 'c', 'd', 'e', 'f', ' ', 'I', 'N', ' ']
        // );
    }

    #[test]
    fn test_complete_no_candidates() {
        // let mut filter = TableFilterComponent::new(KeyConfig::default());
        // filter.input_idx = 2;
        // filter.input = vec!['a', 'n', ' ', 'c', 'd', 'e', 'f', 'g'];
        // filter.completion.update("foo");
        // assert!(filter.complete().is_ok());
        // assert_eq!(filter.input, vec!['a', 'n', ' ', 'c', 'd', 'e', 'f', 'g']);
    }
}
