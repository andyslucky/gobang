use anyhow::Result;
use async_trait::async_trait;
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
use crate::components::command::CommandInfo;
use crate::components::{Drawable, DrawableComponent};
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

    pub fn input_str(&self) -> String {
        self.text_box.input_str()
    }

    pub fn reset(&mut self) {
        self.table = None;
        self.text_box.reset();
    }

    fn update_completion(&mut self) {
        // TODO : reimplement
        // let input = &self
        //     .input
        //     .iter()
        //     .enumerate()
        //     .filter(|(i, _)| i < &self.input_idx)
        //     .map(|(_, i)| i)
        //     .collect::<String>()
        //     .split(' ')
        //     .map(|i| i.to_string())
        //     .collect::<Vec<String>>();
        // self.completion
        //     .update(input.last().unwrap_or(&String::new()));
    }

    fn complete(&mut self) -> anyhow::Result<EventState> {
        // if let Some(candidate) = self.completion.selected_candidate() {
        //     let mut input = Vec::new();
        //     let first = self
        //         .input
        //         .iter()
        //         .enumerate()
        //         .filter(|(i, _)| i < &self.input_idx.saturating_sub(self.completion.word().len()))
        //         .map(|(_, c)| c.to_string())
        //         .collect::<Vec<String>>();
        //     let last = self
        //         .input
        //         .iter()
        //         .enumerate()
        //         .filter(|(i, _)| i >= &self.input_idx)
        //         .map(|(_, c)| c.to_string())
        //         .collect::<Vec<String>>();
        //
        //     let is_last_word = last.first().map_or(false, |c| c == &" ".to_string());
        //
        //     let middle = if is_last_word {
        //         candidate
        //             .chars()
        //             .map(|c| c.to_string())
        //             .collect::<Vec<String>>()
        //     } else {
        //         let mut c = candidate
        //             .chars()
        //             .map(|c| c.to_string())
        //             .collect::<Vec<String>>();
        //         c.push(" ".to_string());
        //         c
        //     };
        //
        //     input.extend(first);
        //     input.extend(middle.clone());
        //     input.extend(last);
        //
        //     self.input = input.join("").chars().collect();
        //     self.input_idx += &middle.len();
        //     if is_last_word {
        //         self.input_idx += 1;
        //     }
        //     self.input_idx -= self.completion.word().len();
        //     self.input_cursor_position += middle
        //         .join("")
        //         .chars()
        //         .map(|c| compute_character_width(&c))
        //         .sum::<u16>();
        //     if is_last_word {
        //         self.input_cursor_position += " ".to_string().width() as u16
        //     }
        //     self.input_cursor_position -= self
        //         .completion
        //         .word()
        //         .chars()
        //         .map(|c| compute_character_width(&c))
        //         .sum::<u16>();
        //     self.update_completion();
        //     return Ok(EventState::Consumed);
        // }
        Ok(EventState::NotConsumed)
    }
}

impl<B : Backend> Drawable<B> for TableFilterComponent {
    fn draw(&mut self, f: &mut Frame<B>, area: Rect, focused: bool) -> Result<()> {
        let a = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Length(7),Constraint::Length(area.width - 7)])
            .split(area);
        let where_literal = Paragraph::new("WHERE").block(Block::default().borders(Borders::ALL));
        // let query = Paragraph::new(Spans::from(vec![
        //     Span::styled(
        //         self.table
        //             .as_ref()
        //             .map_or("-".to_string(), |table| table.name.to_string()),
        //         Style::default().fg(Color::Blue),
        //     ),
        //     Span::from(format!(
        //         " {}",
        //         if focused || !self.input.is_empty() {
        //             self.input.iter().collect::<String>()
        //         } else {
        //             "Enter a SQL expression in WHERE clause to filter records".to_string()
        //         }
        //     )),
        // ]))
        // .style(if focused {
        //     Style::default()
        // } else {
        //     Style::default().fg(Color::DarkGray)
        // })
        // .block(Block::default().borders(Borders::ALL));
        // f.render_widget(query, area);
        //
        // if focused {
        //     self.completion.draw(
        //         f,
        //         area,
        //         false,
        //         (self
        //             .table
        //             .as_ref()
        //             .map_or(String::new(), |table| {
        //                 format!("{} ", table.name.to_string())
        //             })
        //             .width() as u16)
        //             .saturating_add(self.input_cursor_position),
        //         0,
        //     )?;
        // };
        //
        // if focused {
        //     f.set_cursor(
        //         (area.x
        //             + (1 + self
        //                 .table
        //                 .as_ref()
        //                 .map_or(String::new(), |table| table.name.to_string())
        //                 .width()
        //                 + 1) as u16)
        //             .saturating_add(self.input_cursor_position)
        //             .min(area.right().saturating_sub(2)),
        //         area.y + 1,
        //     )
        // }
        f.render_widget(where_literal,a[0]);
        self.text_box.draw(f, a[1], focused)?;
        Ok(())
    }
}

#[async_trait]
impl Component for TableFilterComponent {
    fn commands(&self, _out: &mut Vec<CommandInfo>) {}

    async fn event(&mut self, key: crate::event::Key, message_queue: &mut crate::app::GlobalMessageQueue) -> Result<EventState> {
        if self.text_box.event(key, message_queue).await?.is_consumed() {
            return Ok(Consumed);
        }

        if key == crate::event::Key::Enter {
            return self.complete();
        }

        return Ok(NotConsumed);

        // let input_str: String = self.input.iter().collect();
        //
        // // apply comletion candidates
        // if key == self.key_config.enter {
        //     return self.complete();
        // }
        //
        // self.completion.selected_candidate();
        //
        // match key {
        //     Key::Char(c) => {
        //         self.input.insert(self.input_idx, c);
        //         self.input_idx += 1;
        //         self.input_cursor_position += compute_character_width(&c);
        //         self.update_completion();
        //
        //         Ok(EventState::Consumed)
        //     }
        //     Key::Delete | Key::Backspace => {
        //         if input_str.width() > 0 && !self.input.is_empty() && self.input_idx > 0 {
        //             let last_c = self.input.remove(self.input_idx - 1);
        //             self.input_idx -= 1;
        //             self.input_cursor_position -= compute_character_width(&last_c);
        //             self.completion.update("");
        //         }
        //         Ok(EventState::Consumed)
        //     }
        //     Key::Left => {
        //         if !self.input.is_empty() && self.input_idx > 0 {
        //             self.input_idx -= 1;
        //             self.input_cursor_position = self
        //                 .input_cursor_position
        //                 .saturating_sub(compute_character_width(&self.input[self.input_idx]));
        //             self.completion.update("");
        //         }
        //         Ok(EventState::Consumed)
        //     }
        //     Key::Ctrl('a') => {
        //         if !self.input.is_empty() && self.input_idx > 0 {
        //             self.input_idx = 0;
        //             self.input_cursor_position = 0
        //         }
        //         Ok(EventState::Consumed)
        //     }
        //     Key::Right => {
        //         if self.input_idx < self.input.len() {
        //             let next_c = self.input[self.input_idx];
        //             self.input_idx += 1;
        //             self.input_cursor_position += compute_character_width(&next_c);
        //             self.completion.update("");
        //         }
        //         Ok(EventState::Consumed)
        //     }
        //     Key::Ctrl('e') => {
        //         if self.input_idx < self.input.len() {
        //             self.input_idx = self.input.len();
        //             self.input_cursor_position = self.input_str().width() as u16;
        //         }
        //         Ok(EventState::Consumed)
        //     }
        //     key => self.completion.event(key, message_queue).await,
        // }
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
