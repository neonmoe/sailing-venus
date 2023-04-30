use glam::{IVec2, Vec2};
use std::{
    cmp::Ordering,
    collections::{BinaryHeap, HashMap, HashSet},
};

pub fn find_path(map: &HashMap<IVec2, Vec<IVec2>>, from: Vec2, to: Vec2) -> Option<Vec<Vec2>> {
    let mut from = from.floor().as_ivec2();
    let to = to.floor().as_ivec2();
    let mut path = Vec::new();
    if !map.contains_key(&to) {
        debug_assert!(false, "'to' coordinate ({to}) not on the map");
        return None;
    }
    if !map.contains_key(&from) {
        let mut closest = IVec2::ZERO;
        let mut closest_dist = i32::MAX;
        for &node in map.keys() {
            let delta = node - from;
            let dist_squared = delta.x * delta.x + delta.y * delta.y;
            if dist_squared < closest_dist {
                closest_dist = dist_squared;
                closest = node;
            }
        }
        if !map.contains_key(&closest) {
            debug_assert!(false, "'from' coordinate ({from}) not on the map");
            return None;
        }
        from = closest;
        path.push(from);
    }
    let mut prev = HashMap::with_capacity(map.keys().len());
    prev.insert(from, (0.0, from));
    let mut queue = BinaryHeap::new();
    queue.push(DistSortedCoord {
        pos: from,
        from,
        to,
    });
    let mut processed: HashSet<IVec2> = HashSet::new();
    processed.insert(from);

    while let Some(current) = queue.pop() {
        let curr_dist = prev[&current.pos].0;
        if current.pos == to {
            let mut path = vec![current.pos.as_vec2() + Vec2::ONE * 0.5];
            loop {
                let (prev_dist, prev_pos) = prev[&path[path.len() - 1].floor().as_ivec2()];
                if prev_dist == 0.0 {
                    break;
                }
                path.push(prev_pos.as_vec2() + Vec2::ONE * 0.5);
            }
            path.reverse();
            return Some(path);
        }
        for &neighbor in &map[&current.pos] {
            let dist_to_neighbor =
                curr_dist + (neighbor.as_vec2() - current.pos.as_vec2()).length();
            if !processed.contains(&neighbor) {
                processed.insert(neighbor);
                prev.insert(neighbor, (dist_to_neighbor, current.pos));
                queue.push(DistSortedCoord {
                    pos: neighbor,
                    from,
                    to,
                });
                continue;
            }
            let prev_dist = prev[&neighbor].0;
            if prev_dist > dist_to_neighbor {
                // Shorter path to this neighbor found, replace
                prev.insert(neighbor, (dist_to_neighbor, current.pos));
            }
        }
    }
    debug_assert!(false, "could not find a path from {from} to {to}");
    None
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct DistSortedCoord {
    pos: IVec2,
    from: IVec2,
    to: IVec2,
}

impl PartialOrd for DistSortedCoord {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let get_h = |x: &DistSortedCoord| {
            let diff_to_start = x.pos - x.from;
            let dist_to_start =
                diff_to_start.x * diff_to_start.x + diff_to_start.y * diff_to_start.y;
            let diff_to_end = x.pos - x.to;
            let dist_to_end = diff_to_end.x * diff_to_end.x + diff_to_end.y * diff_to_end.y;
            -(dist_to_start + dist_to_end)
        };
        Some(get_h(self).cmp(&get_h(other)))
    }
}

impl Ord for DistSortedCoord {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}
