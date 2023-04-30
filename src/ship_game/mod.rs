//! The part of the game that happens inside the ship.

use std::collections::VecDeque;

use glam::Vec2;

use crate::math::Aabb2;

pub struct ShipGame {
    pub rooms: Vec<Room>,
    pub characters: Vec<Character>,
    pub selected_character: Option<usize>,
}

pub struct Room {
    pub room_type: RoomType,
    pub position: Vec2,
    /// The area where characters are considered to be in the room. Relative to `position`.
    pub room_bounds: Aabb2,
    /// The area where characters are considered to be working in
    /// this room (unless they're moving through). Relative to `position`.
    pub working_area_bounds: Aabb2,
    pub currently_working_characters: Vec<usize>,
}

pub enum RoomType {
    Empty,
    Navigation,
}

pub struct Character {
    pub position: Vec2,
    pub move_target_queue: VecDeque<Vec2>,
    pub move_speed: f32,
    pub look_dir: Vec2,
    pub current_room: usize,
}

impl ShipGame {
    pub fn new() -> ShipGame {
        ShipGame {
            rooms: vec![
                Room {
                    room_type: RoomType::Navigation,
                    position: Vec2::ZERO,
                    room_bounds: Aabb2::new(Vec2::ONE * -4.0, Vec2::ONE * 4.0),
                    working_area_bounds: Aabb2::new(Vec2::new(1.0, -3.0), Vec2::new(4.0, 3.0)),
                    currently_working_characters: Vec::new(),
                },
                Room {
                    room_type: RoomType::Navigation,
                    position: Vec2::new(0.0, -9.0),
                    room_bounds: Aabb2::new(Vec2::ONE * -4.0, Vec2::ONE * 4.0),
                    working_area_bounds: Aabb2::new(Vec2::ZERO, Vec2::ZERO),
                    currently_working_characters: Vec::new(),
                },
            ],
            characters: vec![Character {
                position: Vec2::new(-2.0, -2.0),
                move_target_queue: VecDeque::new(),
                move_speed: 5.0,
                look_dir: Vec2::new(1.0, 0.0),
                current_room: 0,
            }],
            selected_character: Some(0),
        }
    }

    pub fn click(&mut self, ship_space_point: Vec2) -> bool {
        if let Some(selected_character) = self.selected_character {
            let queue = &mut self.characters[selected_character].move_target_queue;
            queue.clear();
            queue.push_back(ship_space_point);
            true
        } else {
            false
        }
    }

    pub fn update(&mut self, dt: f32) {
        for character in &mut self.characters {
            if !character.move_target_queue.is_empty() {
                let next_move = character.move_target_queue[0];
                let delta = next_move - character.position;
                let delta_length = delta.length();
                let delta_dir = delta / delta_length;
                let mut step_length = character.move_speed * dt;
                if step_length >= delta_length {
                    character.move_target_queue.pop_front();
                    step_length = delta_length;
                }
                character.position += delta_dir * step_length;
                character.look_dir = character.look_dir.lerp(delta_dir, 20.0 * dt);
            }
        }

        for room in &mut self.rooms {
            room.currently_working_characters.clear();
            let bounds = room.room_bounds.offset(room.position);
            let working_bounds = room.working_area_bounds.offset(room.position);
            for (i, c) in self.characters.iter_mut().enumerate() {
                if working_bounds.contains(c.position) {
                    room.currently_working_characters.push(i);
                }
                if bounds.contains(c.position) {
                    c.current_room = i;    
                }
            }
        }
    }
}
