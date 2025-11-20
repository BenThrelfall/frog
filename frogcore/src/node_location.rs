use std::cell::{Cell, RefCell};
use std::f64::consts::TAU;

use serde::{Deserialize, Serialize};

use std::ops::{Add, Mul, Sub};
use crate::units::{Length, Time, METRES};

macro_rules! node_location {
    ($($variant:ident),+) => {

        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub enum NodeLocation {
            $(
                $variant($variant),
            )*
        }

        impl NodeLocation {
            pub fn display_locations(&self, at_time: Time) -> Vec<Point>{
                match self {
                    $(
                        NodeLocation::$variant(inner) => inner.display_locations(at_time),
                    )*
                }
            }

            pub fn distance_to(&self, at_time: Time, from_id: usize, to_id: usize) -> Option<Length> {
                match self {
                    $(
                        NodeLocation::$variant(inner) => inner.distance_to(at_time, from_id, to_id),
                    )*
                }
            }

            pub fn get_adj(&self, node_id: usize) -> Box<dyn Iterator<Item = usize> + '_> {
                match self {
                    $(
                        NodeLocation::$variant(inner) => Box::new(inner.get_adj(node_id)),
                    )*
                }
            }

            /// Number of nodes
            pub fn len(&self) -> usize {
                match self {
                    $(
                        NodeLocation::$variant(inner) => inner.len(),
                    )*
                }
            }
            
            pub fn location(&self, at_time: Time, id: usize) -> Option<Point> {
                    match self {
                        $(
                            NodeLocation::$variant(inner) => inner.location(at_time, id),
                        )*
                    }
                }
            }
    };
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct Edge {
    pub to: usize,

    /// Effective distance
    pub weight: Length,
}

/// Point having Length is currently not correctly integrated.
/// Keep that in mind.
#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct Point {
    pub x: Length,
    pub y: Length,
}

impl Point {
    pub const ZERO: Point = Point {
        x: Length::from_metres(0.0),
        y: Length::from_metres(0.0),
    };

    pub fn from_angle_mag(angle: f64, mag: Length) -> Point {
        Point {
            x: angle.cos() * mag,
            y: angle.sin() * mag,
        }
    }

    pub fn mag(self) -> Length {
        (self.x.powi(2) + self.y.powi(2)).sqrt()
    }

    pub fn normalised(self) -> (f64, f64) {
        let mag = self.mag();
        (self.x / mag, self.y / mag)
    }

    pub fn point_lerp(self, lerp: f64, other: Point) -> Point {
        Point {
            x: self.x * (1. - lerp) + other.x * lerp,
            y: self.y * (1. - lerp) + other.y * lerp,
        }
    }

    /// Returns a copy of `self`
    /// but with magnitude at most `max`
    pub fn clamp_mag(self, max: Length) -> Point {
        if self.mag() > max {
            let (x, y) = self.normalised();
            Point {
                x: x * max,
                y: y * max,
            }
        } else {
            self
        }
    }
}

impl Sub for Point {
    type Output = Point;

    fn sub(self, rhs: Self) -> Self::Output {
        Point {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl Add for Point {
    type Output = Point;

    fn add(self, rhs: Self) -> Self::Output {
        Point {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Mul<f64> for Point {
    type Output = Point;

    fn mul(self, rhs: f64) -> Self::Output {
        Point {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}

node_location!(Graph, Points);

trait ImplNodeLocation {
    fn display_locations(&self, at_time: Time) -> Vec<Point>;
    fn distance_to(&self, at_time: Time, from_id: usize, to_id: usize) -> Option<Length>;

    /// Returns the current location of the node with given id,
    /// if such a concept exists for the underlying model.
    ///
    /// Non-physical models like graphs will return `None`.
    fn location(&self, at_time: Time, id: usize) -> Option<Point>;
    fn get_adj(&self, node_id: usize) -> impl Iterator<Item = usize>;
    fn len(&self) -> usize;
}

/// Graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Graph {
    data: Vec<Vec<Edge>>,
    display: RefCell<Option<Vec<Point>>>,
}

impl Graph {
    pub fn new(edges: Vec<Vec<Edge>>) -> Graph {
        Graph {
            data: edges,
            display: None.into(),
        }
    }
}

impl ImplNodeLocation for Graph {
    fn distance_to(&self, _: Time, from_id: usize, to_id: usize) -> Option<Length> {
        self.data[from_id]
            .iter()
            .find(|x| x.to == to_id)
            .map(|x| x.weight)
    }

    /// Get ids of nodes adjacent to the node with the provided id
    fn get_adj(&self, node_id: usize) -> impl Iterator<Item = usize> {
        self.data[node_id].iter().map(|x| x.to)
    }

    /// Returns the number of nodes
    fn len(&self) -> usize {
        self.data.len()
    }

    fn display_locations(&self, _at_time: Time) -> Vec<Point> {
        let mut display = self.display.borrow_mut();
        let count = self.data.len() as f64;

        display
            .get_or_insert_with(|| {
                let mut positions: Vec<Point> = self
                    .data
                    .iter()
                    .enumerate()
                    .map(|(n, _)| Point {
                        x: (n as f64 * (TAU / count)).cos() * 10000.0 * METRES,
                        y: (n as f64 * (TAU / count)).sin() * 10000.0 * METRES,
                    })
                    .collect();

                for step in 0..1000u32 {
                    let mut updates = vec![(Point::ZERO, false); positions.len()];

                    for (n, pos) in positions.iter().copied().enumerate() {
                        let (point, is_repelling) = &mut updates[n];

                        for (m, other) in positions.iter().copied().enumerate() {
                            if n == m {
                                continue;
                            }

                            let diff = other - pos;
                            if diff.mag() < 1000.0 * METRES {
                                if *is_repelling == false {
                                    *is_repelling = true;
                                    *point = Point::ZERO;
                                }

                                *point = *point - diff * (1000.0 * METRES / diff.mag());
                            }

                            if *is_repelling {
                                continue;
                            }

                            assert!(*is_repelling == false);

                            if self.data[n].iter().find(|x| x.to == m).is_some() {
                                *point = *point + diff;
                            } else {
                                *point = *point - diff * (100_000.0 * METRES / diff.mag().powi(2));
                            }
                        }
                    }
                    for (n, pos) in positions.iter_mut().enumerate() {
                        let (point, is_repelling) = updates[n];

                        *pos = *pos + point * 0.1;

                        if is_repelling {
                            continue;
                        }

                        let x = point.x.metres();
                        let random = (x - x.floor()) * 30.0;

                        assert!(!random.is_nan());

                        *pos = *pos
                            + Point {
                                x: random.cos() * METRES,
                                y: random.sin() * METRES,
                            } * (500.0 - 1.2 * step as f64).max(0.0)
                    }
                }

                for pos in positions.iter_mut() {
                    *pos = *pos * 0.1;
                }

                positions
            })
            .clone()
    }

    fn location(&self, _at_time: Time, _id: usize) -> Option<Point> {
        None
    }
}

/// Points
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Points {
    pub data: Vec<Timepoint>,

    #[serde(skip)]
    counter: Cell<usize>,
}

impl Points {
    pub fn new(data: Vec<Timepoint>) -> Self {
        Self {
            data,
            counter: 0.into(),
        }
    }

    fn move_counter(&self, at_time: Time) {
        while (self.counter.get() != 0 && at_time < self.data[self.counter.get()].time)
            || self
                .data
                .get(self.counter.get() + 1)
                .map(|x| at_time > x.time)
                .unwrap_or(false)
        {
            self.counter.set((self.counter.get() + 1) % self.data.len());
        }
    }
}

/// Two things cannot be in exactly the same place.
/// It breaks at least pathloss calculation.
const MIN_DISTANCE: Length = Length::from_metres(0.05);

impl ImplNodeLocation for Points {
    fn distance_to(&self, at_time: Time, from_id: usize, to_id: usize) -> Option<Length> {
        self.move_counter(at_time);

        let prev = &self.data[self.counter.get()];

        let (a, b) = if self.counter.get() == self.data.len() - 1 {
            (prev.node_points[from_id], prev.node_points[to_id])
        } else if self.counter.get() == 0 && at_time < self.data[0].time {
            (prev.node_points[from_id], prev.node_points[to_id])
        } else {
            let next = &self.data[self.counter.get() + 1];
            let lerp = (at_time - prev.time) / (next.time - prev.time);

            let a = Point::point_lerp(prev.node_points[from_id], lerp, next.node_points[from_id]);
            let b = Point::point_lerp(prev.node_points[to_id], lerp, next.node_points[to_id]);

            (a, b)
        };

        let diff_x = a.x - b.x;
        let diff_y = a.y - b.y;

        if diff_x == 0.0 * METRES && diff_y == 0.0 * METRES {
            return Some(MIN_DISTANCE);
        }

        Some((diff_x.powi(2) + diff_y.powi(2)).sqrt())
    }

    fn get_adj(&self, node_id: usize) -> impl Iterator<Item = usize> {
        (0..self.len()).filter(move |&x| x != node_id)
    }

    fn len(&self) -> usize {
        self.data.first().map(|x| x.node_points.len()).unwrap_or(0)
    }

    fn display_locations(&self, at_time: Time) -> Vec<Point> {
        self.move_counter(at_time);

        if self.counter.get() == self.data.len() - 1 {
            return self.data[self.counter.get()].node_points.clone();
        } else if self.counter.get() == 0 && at_time < self.data[0].time {
            return self.data[self.counter.get()].node_points.clone();
        }

        let prev = &self.data[self.counter.get()];
        let next = &self.data[self.counter.get() + 1];
        let lerp = (at_time - prev.time) / (next.time - prev.time);

        prev.node_points
            .iter()
            .zip(next.node_points.iter())
            .map(|(&a, &b)| Point::point_lerp(a, lerp, b))
            .collect()
    }

    fn location(&self, at_time: Time, id: usize) -> Option<Point> {
        self.move_counter(at_time);

        let prev = &self.data[self.counter.get()];

        let point = if self.counter.get() == self.data.len() - 1 {
            prev.node_points[id]
        } else if self.counter.get() == 0 && at_time < self.data[0].time {
            prev.node_points[id]
        } else {
            let next = &self.data[self.counter.get() + 1];
            let lerp = (at_time - prev.time) / (next.time - prev.time);

            let p = Point::point_lerp(prev.node_points[id], lerp, next.node_points[id]);

            p
        };

        Some(point)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timepoint {
    pub time: Time,
    /// vec index is node id
    pub node_points: Vec<Point>,
}

#[cfg(test)]
mod tests {
    use rand::{rng, Rng};

    use crate::{assert_close, units::METRES};

    use super::*;

    fn get_points(times: usize, nodes: usize) -> Points {
        let data = (0..times)
            .map(|n| Timepoint {
                time: Time::from_seconds(n as f64 * 60.0),
                node_points: (0..nodes)
                    .map(|_| {
                        let x = rng().random::<f64>() * 5000.0 * METRES;
                        let y = rng().random::<f64>() * 5000.0 * METRES;
                        Point { x, y }
                    })
                    .collect(),
            })
            .collect();

        Points::new(data)
    }

    #[test]
    fn points_len() {
        let points = Points::new(vec![Timepoint {
            time: Time::from_seconds(0.0),
            node_points: vec![
                Point {
                    x: 2.0 * METRES,
                    y: 10.0 * METRES,
                },
                Point {
                    x: 5.0 * METRES,
                    y: 7.5 * METRES,
                },
            ],
        }]);

        assert_eq!(points.len(), 2);

        let points = Points::new(vec![Timepoint {
            time: Time::from_seconds(0.0),
            node_points: (0..25)
                .map(|_| Point {
                    x: 2.0 * METRES,
                    y: 60.3 * METRES,
                })
                .collect(),
        }]);

        assert_eq!(points.len(), 25);
    }

    #[test]
    fn display_real_agree() {
        let points = get_points(5, 15);

        let at_time = Time::from_seconds(145.3);

        let now_points = points.display_locations(at_time);

        for (n, point_a) in now_points.iter().enumerate() {
            for (m, point_b) in now_points.iter().enumerate() {
                let forward = points.distance_to(at_time, n, m).unwrap();
                let reverse = points.distance_to(at_time, m, n).unwrap();

                let sq_dist = (point_a.x - point_b.x).powi(2) + (point_a.y - point_b.y).powi(2);
                let dist = sq_dist.sqrt().max(MIN_DISTANCE);

                println!("{n} - {point_a:?} ; {m} - {point_b:?}");
                assert_close(forward, reverse);
                println!("HERE");
                assert_close(dist, forward);
            }
        }
    }
}
