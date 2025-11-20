use std::f64::consts::TAU;

use rand::{seq::IndexedRandom, Rng};
use rand_chacha::ChaCha12Rng;
use rand_distr::{Distribution, Normal};
use serde::{Deserialize, Serialize};

use crate::{
    node_location::Timepoint,
    node_location::Point,
    units::{Length, Speed, Time, Unit, METRES, SECONDS}, utility::n_min,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WonderingNodes {
    pub side_len: Length,

    /// Position states will be uniformly randomly distributed across this time period.
    pub movement_timespan: Time,

    /// A node will move at most this distance between frames
    pub wonder_speed: Speed,
}

impl WonderingNodes {

    /// Positions across time `[mobile, stationary]`
    pub(super) fn generate(
        self,
        mobile_count: usize,
        stationary_count: usize,
        rng: &mut ChaCha12Rng,
    ) -> Vec<Timepoint> {
        let WonderingNodes {
            side_len,
            movement_timespan,
            wonder_speed,
        } = self;

        let mut time = 0.0 * SECONDS;
        let delta_time = 10.0 * SECONDS;

        let mut map = Vec::new();

        let stationary_points = pos_random_square(stationary_count, side_len, rng);
        let mut mobile_points = pos_random_square(mobile_count, side_len, rng);
        let mut directions: Vec<_> = (0..mobile_count)
            .map(|_| rng.random_range(0.0..TAU))
            .collect();

        while time < movement_timespan {
            for (point, dir) in mobile_points.iter_mut().zip(directions.iter_mut()) {
                if rng.random_bool(0.1) {
                    *dir = rng.random_range(0.0..TAU);
                }

                *point = *point + Point::from_angle_mag(*dir, wonder_speed * delta_time);
                point.x = point.x.min(side_len).max(0.0 * METRES);
                point.y = point.y.min(side_len).max(0.0 * METRES);
            }

            map.push(Timepoint {
                time,
                node_points: mobile_points
                    .iter()
                    .cloned()
                    .chain(stationary_points.iter().cloned())
                    .collect(),
            });

            time = time + delta_time;
        }

        map
    }
}

/// A number of indepedent frames of positions.
/// No limits other than `side_len`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndependentPositionFrames {
    pub side_len: Length,
    /// At least 1
    /// Number of position states the nodes will linearly move between.
    pub position_count: usize,
    /// Position states will be uniformly distributed across this time period.
    pub movement_timespan: Time,
}

impl IndependentPositionFrames {
    /// Positions across time `[mobile, stationary]`
    pub(super) fn generate(
        self,
        mobile_count: usize,
        stationary_count: usize,
        rng: &mut ChaCha12Rng,
    ) -> Vec<Timepoint> {
        let IndependentPositionFrames {
            side_len,
            position_count,
            movement_timespan,
        } = self;

        let mut pos_times: Vec<_> = (0..position_count)
            .map(|_| movement_timespan.map(|x| rng.random_range(0.0..x)))
            .collect();

        pos_times.sort_by(|a, b| a.partial_cmp(b).expect("Should not be NaN"));

        let stationary_points = pos_random_square(stationary_count, side_len, rng);

        let map: Vec<Timepoint> = pos_times
            .iter()
            .map(|t| Timepoint {
                time: *t,
                node_points: pos_random_square(mobile_count, side_len, rng)
                    .into_iter()
                    .chain(stationary_points.iter().cloned())
                    .collect(),
            })
            .collect();

        map
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PathwayMovement {
    pub side_len: Length,
    pub mean_movement_speed: Speed,
    pub std_movement_speed: Speed,
    pub nth_pathway_chance: Vec<f64>,
}

impl PathwayMovement {
    /// Positions across time as `[isolated_points, active_key_points, people]`
    pub(super) fn generate(
        self,
        isolated_points_count: usize,
        active_key_points: usize,
        passive_key_points: usize,
        people_count: usize,
        end_time: Time,
        rng: &mut ChaCha12Rng,
    ) -> Vec<Timepoint> {

        let PathwayMovement {
            side_len,
            mean_movement_speed,
            std_movement_speed,
            nth_pathway_chance,
        } = self;

        let key_points_count = active_key_points + passive_key_points;

        let key_points = pos_random_square(key_points_count, side_len, rng);

        let mut point_distances = vec![vec![0.0 * METRES; key_points_count]; key_points_count];
        let mut path_exists = vec![vec![false; key_points_count]; key_points_count];

        for (n, point_a) in key_points.iter().copied().enumerate() {
            for (m, point_b) in key_points.iter().copied().enumerate() {
                if m <= n {
                    continue;
                }

                let dist = (point_a - point_b).mag();
                point_distances[n][m] = dist;
                point_distances[m][n] = dist;
            }
        }

        for (n, _) in key_points.iter().copied().enumerate() {
            let closest_points = n_min(&point_distances[n], nth_pathway_chance.len() + 1);
            for (pos, index) in closest_points.into_iter().skip(1).enumerate() {
                if rng.random_bool(nth_pathway_chance[pos]) {
                    path_exists[n][index] = true;
                    path_exists[index][n] = true;
                }
            }
        }

        let adjacency_list: Vec<Vec<usize>> = (0..key_points.len())
            .map(|n| {
                path_exists[n]
                    .iter()
                    .enumerate()
                    .filter_map(|(index, is_path)| is_path.then_some(index))
                    .collect()
            })
            .collect();

        let speed_dist =
            Normal::new(mean_movement_speed.inner(), std_movement_speed.inner()).unwrap();

        let mut people: Vec<PathPerson> = (0..people_count)
            .map(|_| {
                let key_a = rng.random_range(0..key_points_count);
                let key_b = adjacency_list[key_a].choose(rng).copied();

                let start_point = if let Some(index) = key_b {
                    let lerp: f64 = rng.random();
                    key_points[key_a].point_lerp(lerp, key_points[index])
                } else {
                    key_points[key_a]
                };

                PathPerson {
                    pos: start_point,
                    destination: key_a,
                    speed: speed_dist.sample(rng).max(0.0).into(),
                }
            })
            .collect();

        let mut people_positions = Vec::new();

        let timestep = 10.0 * SECONDS;
        let mut time = 0.0 * SECONDS;

        let isolated_points = pos_random_square(isolated_points_count, side_len, rng);
        let static_points: Vec<Point> = isolated_points
            .into_iter()
            .chain(
                key_points[..active_key_points].iter().copied()
            )
            .collect();

        while time < end_time {
            people_positions.push(Timepoint {
                time,
                node_points: static_points
                    .iter()
                    .copied()
                    .chain(people.iter().map(|x| x.pos))
                    .collect(),
            });

            people.iter_mut().for_each(|person| {
                let dest_pos = key_points[person.destination];
                person.pos =
                    person.pos + (dest_pos - person.pos).clamp_mag(person.speed * timestep);

                if (person.pos - dest_pos).mag() < 4.0 * METRES {
                    person.destination =
                        *adjacency_list[person.destination].choose(rng).unwrap();
                }
            });

            time = time + timestep;
        }

        people_positions
    }
}

/// Distributes `count` points uniformly at random in the region `0..side_len` for both x and y
pub(super) fn pos_random_square(
    count: usize,
    side_len: Length,
    rng: &mut ChaCha12Rng,
) -> Vec<Point> {
    (0..count)
        .map(|_| Point {
            x: rng.random::<f64>() * side_len,
            y: rng.random::<f64>() * side_len,
        })
        .collect()
}

pub fn reorder_locations(map: Vec<Timepoint>, projection: Vec<usize>) -> Vec<Timepoint> {
    map.iter()
        .map(|Timepoint { time, node_points }| Timepoint {
            time: *time,
            node_points: projection.iter().map(|index| node_points[*index]).collect(),
        })
        .collect()
}

struct PathPerson {
    pos: Point,
    destination: usize,
    speed: Speed,
}