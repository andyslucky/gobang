use anyhow::Result;
use async_trait::async_trait;
use log::{debug, error};
use tui::{
    backend::Backend,
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Clear, List, ListItem, ListState},
};

use crate::app::{AppMessage, SharedPool};
use crate::components::command::CommandInfo;
use crate::config::KeyConfig;

use super::{Component, EventState, MovableComponent};

#[async_trait]
/// A FilterableCompletionSource abstracts the completion logic for a completion component.
/// This allows each sql pool vendor/parent component to customize completion options to fit the context
/// of the user's current action. Many vendors have their own unique set of keywords, this allows
pub trait FilterableCompletionSource : Send + Sync {
    /// Gets completion items for the last word part. Does not use current context to optimize suggestions
    /// and suggestion order. This will be coming in a future update
    async fn suggested_completion_items(&self, last_word_part: &String) -> anyhow::Result<Vec<String>>;
}

struct DefaultFilterableCompletionSource {
    sql_key_words: Vec<String>,
}

impl DefaultFilterableCompletionSource {
    fn new() -> Self {
        Self{
            sql_key_words:vec![
                "IN", "AND", "OR", "NOT", "NULL", "IS", "SELECT", "INSERT", "UPDATE", "DELETE", "FROM", "LIMIT", "WHERE", "LIKE"
            ].iter().map(|s| String::from(*s)).collect()
        }
    }
}

#[async_trait]
impl FilterableCompletionSource for DefaultFilterableCompletionSource {
    async fn suggested_completion_items(&self, last_word_part: &String) -> Result<Vec<String>> {
        let pattern_res = regex::Regex::new(format!("(?i)^{}", last_word_part).as_str());
        if let Err(e) = &pattern_res {
            error!("Error compiling pattern {}",e);
            return Err(e.clone().into());
        }
        let patt = pattern_res.unwrap();
        let candidates = self.sql_key_words.iter()
            .filter(|kw| patt.is_match(kw.as_str()))
            .map(|kw| kw.clone())
            .collect();
        debug!("Filtered candidates {:?}", candidates);
        return Ok(candidates);
    }
}


pub struct CompletionComponent {
    key_config: KeyConfig,
    state: ListState,
    word: String,
    candidates: Vec<String>,
    completion_source : Box<dyn FilterableCompletionSource>

    // shared_pool : SharedPool
}

impl CompletionComponent {
    pub fn new(key_config: KeyConfig, word: impl Into<String>, all: bool) -> Self {
        Self {
            key_config,
            state: ListState::default(),
            word: word.into(),
            candidates: vec![],
            completion_source : Box::new(DefaultFilterableCompletionSource::new())
        }
    }


    pub async fn update<S: Into<String>>(&mut self, word_part: S) {
        self.word = word_part.into();
        self.state.select(None);
        let candidates_res = self.completion_source.suggested_completion_items(&self.word).await;
        if let Err(e) = &candidates_res {
            error!("Error fetching completion candidates {}",e);
        } else if let Ok(candidates) = &candidates_res {
            debug!("Filtered candidates {:?}", candidates);
            self.candidates = candidates.clone();
            if !self.candidates.is_empty() {
                self.state.select(Some(0));
            }
        }
    }

    fn change_selection(&mut self, offset: i32) {
        if let Some(i) = self.state.selected() {
            let new_selected_index = (i as i32 + offset) as usize;
            if new_selected_index >= 0 && new_selected_index < self.candidates.len() {
                self.state.select(Some(new_selected_index));
            }
        }
    }

    fn next(&mut self) {
        self.change_selection(1);
    }

    fn previous(&mut self) {
        self.change_selection(-1);
    }


    pub fn selected_candidate(&self) -> Option<String> {
        if let Some(index) = self.state.selected() {
            Some(self.candidates[index].clone())
        } else {
            None
        }
    }

    pub fn word(&self) -> String {
        self.word.to_string()
    }
}

impl MovableComponent for CompletionComponent {
    fn draw<B: Backend>(
        &mut self,
        f: &mut Frame<B>,
        area: Rect,
        _focused: bool,
        x: u16,
        y: u16,
    ) -> Result<()> {
        if !self.word.is_empty() {
            let width = 30;
            let candidates = self
                .candidates
                .iter()
                .map(|c| ListItem::new(c.to_string()))
                .collect::<Vec<ListItem>>();
            let cand_len = candidates.len();
            if candidates.is_empty() {
                return Ok(());
            }
            let candidate_list = List::new(candidates)
                .block(Block::default().borders(Borders::ALL))
                .highlight_style(Style::default().bg(Color::Blue))
                .style(Style::default());

            let area = Rect::new(
                x, y,
                width
                    .min(f.size().width)
                    .min(f.size().right().saturating_sub(area.x + x)),
                (cand_len.min(5) as u16 + 2)
                    .min(f.size().bottom().saturating_sub(area.y + y + 2)),
            );
            f.render_widget(Clear, area);
            f.render_stateful_widget(candidate_list, area, &mut self.state);
        }
        Ok(())
    }
}

#[async_trait]
impl Component for CompletionComponent {
    fn commands(&self, _out: &mut Vec<CommandInfo>) {}

    async fn event(&mut self, key: crate::event::Key, _message_queue: &mut crate::app::GlobalMessageQueue) -> Result<EventState> {
        if key == self.key_config.move_down {
            self.next();
            return Ok(EventState::Consumed);
        } else if key == self.key_config.move_up {
            self.previous();
            return Ok(EventState::Consumed);
        }
        Ok(EventState::NotConsumed)
    }

    fn is_visible(&self) -> bool {
        return !self.word.is_empty();
    }

    fn reset(&mut self) {
        self.word = "".to_string();
        self.state.select(None);
    }
}

// #[cfg(test)]
// mod test {
//     use super::{CompletionComponent, KeyConfig};
//
//     #[test]
//     fn test_filterd_candidates_lowercase() {
//         assert_eq!(
//             CompletionComponent::new(KeyConfig::default(), "an", false)
//                 .filtered_candidates()
//                 .collect::<Vec<&String>>(),
//             vec![&"AND".to_string()]
//         );
//     }
//
//     #[test]
//     fn test_filterd_candidates_uppercase() {
//         assert_eq!(
//             CompletionComponent::new(KeyConfig::default(), "AN", false)
//                 .filtered_candidates()
//                 .collect::<Vec<&String>>(),
//             vec![&"AND".to_string()]
//         );
//     }
//
//     #[test]
//     fn test_filterd_candidates_multiple_candidates() {
//         assert_eq!(
//             CompletionComponent::new(KeyConfig::default(), "n", false)
//                 .filtered_candidates()
//                 .collect::<Vec<&String>>(),
//             vec![&"NOT".to_string(), &"NULL".to_string()]
//         );
//
//         assert_eq!(
//             CompletionComponent::new(KeyConfig::default(), "N", false)
//                 .filtered_candidates()
//                 .collect::<Vec<&String>>(),
//             vec![&"NOT".to_string(), &"NULL".to_string()]
//         );
//     }
// }
