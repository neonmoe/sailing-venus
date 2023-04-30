//! The thing shown on the dashboard in-game.

use crate::ship_game::{ShipGame, Task};
use sdl2::rect::{Point, Rect};
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Button {
    Tab(usize),
    TaskPicker(Task),
    TaskAssigner { time: usize, character: usize },
}

pub enum Tab {
    Navigation,
    Schedule,
    Deliveries,
    GameSettings,
}

pub struct Interface {
    pub buttons: HashMap<Button, Rect>,
    /// The inner screen area.
    pub screen_area: Rect,
    /// The area of the whole UI.
    pub safe_area: Rect,
    pub hovered_tab: Option<usize>,
    pub tab: Option<Tab>,
    pub selected_task: Task,
}

impl Interface {
    pub fn new() -> Interface {
        Interface {
            buttons: HashMap::new(),
            screen_area: Rect::new(0, 0, 0, 0),
            safe_area: Rect::new(0, 0, 0, 0),
            hovered_tab: None,
            tab: None,
            selected_task: Task::Sleep,
        }
    }

    pub fn click(&mut self, position: Point, ship_game: &mut ShipGame, held: bool) {
        let mut open_tab = None;
        for (button, button_area) in &self.buttons {
            if button_area.contains_point(position) {
                match button {
                    Button::Tab(i) if !held => {
                        open_tab = Some(*i);
                        break;
                    }
                    Button::TaskPicker(task) if !held => {
                        self.selected_task = *task;
                    }
                    Button::TaskAssigner { time, character } => {
                        ship_game.characters[*character].schedule[*time] = self.selected_task;
                    }
                    _ => {}
                }
            }
        }
        if let Some(i) = open_tab {
            self.open_tab(i);
        }
    }

    pub fn open_tab(&mut self, tab_index: usize) {
        let tab = match tab_index {
            0 => Tab::Navigation,
            1 => Tab::Schedule,
            2 => Tab::Deliveries,
            3 => Tab::GameSettings,
            _ => unreachable!(),
        };
        self.tab = Some(tab);
    }
}
