use crate::{
    math::Aabb2,
    renderer::{gltf, Renderer},
};
use glam::{IVec2, Vec2, Vec4, Vec4Swizzles};
use std::collections::{HashMap, HashSet};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomType {
    Navigation,
    Sails,
}

impl Room {
    pub fn new(
        renderer: &Renderer,
        room_type: RoomType,
        position: Vec2,
        // TODO(opt): replace inner Vec with a u8 ("neighbor exists" bits)
        pathfinding_neighbors: &mut HashMap<IVec2, Vec<IVec2>>,
    ) -> Room {
        let pathfinding_nodes = match room_type {
            RoomType::Navigation | RoomType::Sails => get_pathfinding_nodes(&renderer.room),
        };
        let ipos = position.floor().as_ivec2();
        let nodes_set: HashSet<IVec2> =
            HashSet::from_iter(pathfinding_nodes.iter().map(|n| *n + ipos));
        for node in &pathfinding_nodes {
            let node = ipos + *node;
            let mut neighbors = Vec::with_capacity(8);
            for yo in -1..=1 {
                for xo in -1..=1 {
                    if yo == 0 && xo == 0 {
                        continue;
                    }
                    let neighbor = node + IVec2::new(xo, yo);
                    if nodes_set.contains(&neighbor)
                        || pathfinding_neighbors.contains_key(&neighbor)
                    {
                        neighbors.push(neighbor);
                    }
                }
            }
            if let Some(existing_neighbors) = pathfinding_neighbors.get_mut(&node) {
                for new_neighbor in neighbors {
                    if !existing_neighbors.contains(&new_neighbor) {
                        existing_neighbors.push(new_neighbor);
                    }
                }
            } else {
                pathfinding_neighbors.insert(node, neighbors);
            }
        }
        Room {
            room_type,
            position,
            room_bounds: Aabb2::new(Vec2::ONE * -4.0, Vec2::ONE * 4.0),
            working_area_bounds: Aabb2::new(Vec2::new(1.0, -3.0), Vec2::new(4.0, 3.0)),
            currently_working_characters: Vec::new(),
        }
    }
}

fn get_pathfinding_nodes(gltf: &gltf::Gltf) -> Vec<IVec2> {
    let mut nodes = Vec::new();
    for node in gltf.get_node_transforms() {
        if node.name.starts_with("Empty") {
            let pos = (node.transform * Vec4::new(0.0, 0.0, 0.0, 1.0)).xz();
            nodes.push(pos.floor().as_ivec2());
        }
    }
    nodes
}
