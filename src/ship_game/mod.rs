//! The part of the game that happens inside the ship.

use crate::renderer::Renderer;
use glam::{IVec2, Vec2};
use std::collections::{HashMap, VecDeque};

mod pathfinding;
mod room;

pub use room::*;

pub type PathfindingMap = HashMap<IVec2, Vec<IVec2>>;
const SLEEPING_COORDS: Vec2 = Vec2::new(-2.5, -9.5);
const MAX_SHIP_SPEED: f32 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Task {
    Sleep,
    Work,
}

pub struct ShipGame {
    /// The time in the in-game world, counted in days. One day is a minute in real-time.
    pub world_time: f32,
    pub rooms: Vec<Room>,
    pub characters: Vec<Character>,
    pub selected_character: Option<usize>,
    /// Coordinate -> neighbor coordinates
    pub pf_map: PathfindingMap,
    pub locations: Vec<(&'static str, Vec2)>,
    pub current_location: Vec2,
    pub current_target: Vec2,
    pub current_heading: Vec2,
    pub current_ship_speed: f32,
    pub deliveries: Vec<(&'static str, Vec2, bool)>,
}

pub struct Character {
    pub position: Vec2,
    pub move_target_queue: VecDeque<Vec2>,
    pub move_speed: f32,
    pub look_dir: Vec2,
    pub current_room: usize,
    pub schedule: [Task; 12],
    pub job: Job,
}

impl Character {
    fn pathfind_to(&mut self, map: &PathfindingMap, to: Vec2) {
        if let Some(path) = pathfinding::find_path(map, self.position, to) {
            self.move_target_queue.extend(path);
        } else {
            debug_assert!(false, "{:?} can't find path to {:?}", &self.job, to);
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Job {
    Navigator,
    Sailor,
    #[doc(hidden)]
    Count,
}

impl ShipGame {
    pub fn new(renderer: &Renderer) -> ShipGame {
        let mut pathfinding_neighbors = HashMap::new();
        let rooms = vec![
            Room::new(
                &renderer,
                RoomType::Navigation,
                Vec2::new(0.0, -4.0),
                &mut pathfinding_neighbors,
            ),
            Room::new(
                &renderer,
                RoomType::Sails,
                Vec2::new(0.0, 5.0),
                &mut pathfinding_neighbors,
            ),
        ];
        ShipGame {
            world_time: 0.0,
            rooms,
            pf_map: pathfinding_neighbors,
            characters: vec![
                Character {
                    position: SLEEPING_COORDS,
                    move_target_queue: VecDeque::new(),
                    move_speed: 5.0,
                    look_dir: Vec2::new(1.0, 0.0),
                    current_room: 0,
                    schedule: [Task::Sleep; 12],
                    job: Job::Navigator,
                },
                Character {
                    position: SLEEPING_COORDS,
                    move_target_queue: VecDeque::new(),
                    move_speed: 5.0,
                    look_dir: Vec2::new(1.0, 0.0),
                    current_room: 0,
                    schedule: [Task::Sleep; 12],
                    job: Job::Sailor,
                },
            ],
            selected_character: Some(0),
            locations: vec![
                ("Morning Star Post Union", Vec2::new(0.0, 0.0)),
                ("Terraforming Station Alpha-Beta", Vec2::new(100.0, 50.0)),
                ("Firefly Space Station", Vec2::new(75.0, 75.0)),
                ("A Mineral Miner's Shack", Vec2::new(-25.0, -50.0)),
            ],
            current_location: Vec2::ZERO,
            current_target: Vec2::ZERO,
            current_heading: Vec2::new(1.0, 0.0),
            current_ship_speed: 0.0,
            deliveries: vec![
                ("Mineral refiner", Vec2::new(-25.0, -50.0), false),
                ("Rocket fuel", Vec2::new(75.0, 75.0), false),
            ],
        }
    }

    pub fn update(&mut self, dt: f32) {
        let dt = dt.min(1.0 / 30.0);
        self.world_time += dt / 60.0;
        let current_hour = (self.world_time * 12.0).floor() as usize % 12;
        for character in &mut self.characters {
            if character.move_target_queue.is_empty() {
                // Not doing anything, queue something to do
                match character.schedule[current_hour] {
                    Task::Sleep => {
                        if character.position != SLEEPING_COORDS {
                            character.pathfind_to(&self.pf_map, SLEEPING_COORDS);
                        }
                    }
                    Task::Work => {
                        let room = self.rooms.iter().find(|room| match character.job {
                            Job::Sailor => room.room_type == RoomType::Sails,
                            Job::Navigator => room.room_type == RoomType::Navigation,
                            _ => false,
                        });
                        if let Some(room) = room {
                            if !room
                                .working_area_bounds
                                .offset(room.position)
                                .contains(character.position)
                            {
                                let target = room.position
                                    + (room.working_area_bounds.min + room.working_area_bounds.max)
                                        / 2.0;
                                character.pathfind_to(&self.pf_map, target);
                            }
                        } else {
                            character.pathfind_to(&self.pf_map, SLEEPING_COORDS);
                        }
                    }
                }
            } else {
                let next_move = character.move_target_queue[0];
                let delta = next_move - character.position;
                let delta_length = delta.length();
                let delta_dir = delta.normalize_or_zero();
                let step_length = character.move_speed * dt;
                if step_length >= delta_length {
                    character.move_target_queue.pop_front();
                    character.position = next_move;
                } else {
                    character.position += delta_dir * step_length;
                }
                if delta_dir.length() > 0.0 {
                    character.look_dir = character.look_dir.lerp(delta_dir, 20.0 * dt);
                }
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
            let ship_loc_delta = self.current_target - self.current_location;
            if room.room_type == RoomType::Navigation {
                let direction = ship_loc_delta.normalize_or_zero();
                if direction.length_squared() > 0.0 {
                    self.current_heading = self.current_heading.lerp(direction, 10.0 * dt);
                }
            }
            if room.room_type == RoomType::Sails {
                let acceleration =
                    room.currently_working_characters.len() as f32 / 20.0 * MAX_SHIP_SPEED;
                if acceleration > 0.0 {
                    self.current_ship_speed =
                        (self.current_ship_speed + acceleration * dt).min(MAX_SHIP_SPEED);
                } else {
                    self.current_ship_speed =
                        (self.current_ship_speed - MAX_SHIP_SPEED / 10.0 * dt).max(0.0);
                }
                let step = self.current_ship_speed * self.current_heading * dt;
                if step.length_squared() >= ship_loc_delta.length_squared() {
                    self.current_location = self.current_target;
                } else {
                    self.current_location += step;
                }
                for delivery in &mut self.deliveries {
                    if delivery.1 == self.current_location {
                        delivery.2 = true;
                    }
                }
            }
        }
    }
}
