use std::{
    iter,
    ops::{Add, Div, Mul, Range, Sub},
    path::PathBuf,
};

use clap::{Parser, command};
use frogcore::{
    scenario::{ScenarioIdentity, generation::{
        ScenarioGenerator::*,
        messaging::IndependentRandomMessaging,
        positioning::{IndependentPositionFrames, PathwayMovement, WonderingNodes},
    }},
    sim_file::write_file,
    simulation::models::{AdjustedFreeSpacePathLoss, PairWiseCaptureEffect},
    units::{KM, METRES, MINS, MPS},
};
use rand::{Rng, SeedableRng, rng};
use rand_chacha::ChaCha12Rng;
use rand_distr::Normal;

#[derive(Parser, Debug)]
#[command()]
struct Args {
    output: PathBuf,

    #[arg(long)]
    seed: Option<u64>,

    /// Skip generating scenarios with a given chance
    #[arg(long)]
    skip: Option<f64>,
}

macro_rules! make_params {
    {
        $name: ident { $( $var_name: ident : $var_type: ty  ),+ $(,)? } $data_name: ident
    } => {

            pub struct $data_name {
                $(
                    pub $var_name : $var_type,
                )+
            }

            pub struct $name {
                $(
                    pub $var_name : ParamVec<$var_type>,
                )+
            }

            impl $name {
                pub fn values(&self) -> $data_name {
                    $data_name {
                        $(
                            $var_name: self.$var_name.current()
                        ),+
                    }
                }

                pub fn next(&mut self) -> bool {
                    $(self.$var_name.next() || )+ false
                }

                pub fn len(&self) -> usize {
                    $(self.$var_name.data.len() * )+ 1
                }
            }

    };
}

mod params {
    use super::ParamVec;
    use frogcore::units::*;

    make_params! {
        PsudoSpatialGraphParams {
            nodes: usize,
            n_connections: usize,
            message_count: usize,
            messaging_timespan: Time,
            mean_message_size: f64,
            std_message_size: f64,
            broadcast_chance: f64,
            directed: bool,
        }
        PsudoSpatialGraphData
    }

    make_params! {
        WonderingRandomSquareParams{
            side_len: Length,
            node_count: usize,
            movement_speed: Speed,
            gateway_count: usize,
            gateways_move: bool,
            message_count: usize,
            timespan: Time,
            mean_message_size: f64,
            std_message_size: f64,
            broadcast_chance: f64,
            path_loss_exp: f64,
            emergency_time_coef: Option<f64>,
            with_fading: bool,
        }
        WonderingRandomSquareData
    }

    make_params! {
        RandomSquareParams{
                side_len: Length,
                node_count: usize,
                position_count: usize,
                gateway_count: usize,
                gateways_move: bool,
                message_count: usize,
                timespan: Time,
                mean_message_size: f64,
                std_message_size: f64,
                broadcast_chance: f64,
                path_loss_exp: f64,
                with_fading: bool,
        }
        RandomSquareData
    }

    make_params! {
        PathwaysOneParams {

            passive_key_points: usize,
            radio_key_points: usize,
            gateway_key_points: usize,
            isolated_points_count: usize,
            isolated_gateway_count: usize,
            people_count: usize,

            nth_pathway_chance: Vec<f64>,
            mean_movement_speed: Speed,
            std_movement_speed: Speed,
            side_len: Length,

            message_count: usize,
            messaging_timespan: Time,
            mean_message_size: f64,
            std_message_size: f64,
            broadcast_chance: f64,

            emergency_time_coef: Option<f64>,

            path_loss_exp: f64,
        }
        PathwaysOneData
    }
}

fn main() {
    let Args { output, seed, skip } = Args::parse();

    let seed = seed.unwrap_or_else(|| rng().random());

    let mut seeding_rng = ChaCha12Rng::seed_from_u64(seed);

    let mut all_scenarios = Vec::new();

    all_scenarios.append(&mut spatial_graphs(&mut seeding_rng));
    all_scenarios.append(&mut random_square(&mut seeding_rng));
    all_scenarios.append(&mut pathways_one(&mut seeding_rng));
    all_scenarios.append(&mut wondering_random_square(&mut seeding_rng));

    println!("{}", all_scenarios.len());

    if let Some(skip_prob) = skip {
        all_scenarios.retain(|_| seeding_rng.random_bool(1.0 - skip_prob));
        println!("{}", all_scenarios.len());
    }

    write_file(output, all_scenarios, true).unwrap();
}

fn spatial_graphs(seeding_rng: &mut ChaCha12Rng) -> Vec<ScenarioIdentity> {
    let mut output = Vec::new();

    let mut params = params::PsudoSpatialGraphParams {
        nodes: range(15..30, 1),
        n_connections: fixed(3),
        message_count: range(50..500, 100),
        messaging_timespan: range(1.0 * MINS..5.1 * MINS, 1.0 * MINS),
        mean_message_size: linspace(20.0, 200.0, 5),
        std_message_size: vec![10.0, 50.0].into(),
        broadcast_chance: range(0.1..0.8, 0.2),
        directed: flip(),
    };

    println!("Spatial Graphs: {}", params.len());

    loop {
        let params::PsudoSpatialGraphData {
            nodes,
            n_connections,
            message_count,
            messaging_timespan,
            mean_message_size,
            std_message_size,
            broadcast_chance,
            directed,
        } = params.values();

        let scenario = ScenarioIdentity::Generated {
            generator: PsudoSpatialGraph {
                nodes,
                n_connections,
                messaging: IndependentRandomMessaging {
                    message_count,
                    messaging_timespan,
                    mean_message_size,
                    std_message_size,
                    broadcast_chance,
                    gateway_priority: 0.0,
                },
                directed,
            },
            seed: seeding_rng.random(),
        };

        output.push(scenario);

        if !params.next() {
            break;
        }
    }

    output
}

fn random_square(seeding_rng: &mut ChaCha12Rng) -> Vec<ScenarioIdentity> {
    let mut output = Vec::new();

    let mut params = params::RandomSquareParams {
        side_len: linspace(500.0 * METRES, 5.0 * KM, 6),
        node_count: vec![10, 20, 50].into(),
        position_count: fixed(2),
        timespan: vec![2.0 * MINS, 5.0 * MINS, 10.0 * MINS, 30.0 * MINS].into(),
        message_count: vec![50, 200, 500].into(),
        mean_message_size: vec![20.0, 160.0].into(),
        std_message_size: vec![5.0, 60.0].into(),
        broadcast_chance: vec![0.1, 0.5, 0.9].into(),
        path_loss_exp: linspace(2.8, 4.2, 5),
        gateway_count: fixed(1),
        gateways_move: fixed(false),
        with_fading: flip(),
    };

    println!("Random Squares: {}", params.len());

    loop {
        let params::RandomSquareData {
            side_len,
            node_count,
            position_count,
            message_count,
            mean_message_size,
            std_message_size,
            broadcast_chance,
            path_loss_exp,
            timespan,
            gateway_count,
            gateways_move,
            with_fading,
        } = params.values();

        let scenario = ScenarioIdentity::Generated {
            generator: RandomSquare {
                node_count,
                messaging: IndependentRandomMessaging {
                    message_count,
                    messaging_timespan: timespan,
                    mean_message_size,
                    std_message_size,
                    broadcast_chance,
                    gateway_priority: 0.0,
                },
                model: if with_fading {
                    PairWiseCaptureEffect::default()
                        .with_pathloss(
                            AdjustedFreeSpacePathLoss::new(path_loss_exp, 0.0.into()).into(),
                        )
                        .with_fading(Normal::new(0.0, 4.0).unwrap())
                        .into()
                } else {
                    PairWiseCaptureEffect::default()
                        .with_pathloss(
                            AdjustedFreeSpacePathLoss::new(path_loss_exp, 0.0.into()).into(),
                        )
                        .into()
                },
                gateway_count,
                gateways_move,
                positioning: IndependentPositionFrames {
                    side_len,
                    position_count,
                    movement_timespan: timespan * 2.0,
                },
            },
            seed: seeding_rng.random(),
        };

        output.push(scenario);

        if !params.next() {
            break;
        }
    }

    output
}

fn wondering_random_square(seeding_rng: &mut ChaCha12Rng) -> Vec<ScenarioIdentity> {
    let mut output = Vec::new();

    let mut params = params::WonderingRandomSquareParams {
        side_len: linspace(1.0 * KM, 10.0 * KM, 10),
        node_count: vec![10, 20, 50].into(),
        timespan: vec![2.0 * MINS, 5.0 * MINS, 10.0 * MINS, 30.0 * MINS].into(),
        movement_speed: fixed(1.35 * MPS),
        message_count: vec![50, 200, 500].into(),
        mean_message_size: vec![20.0, 160.0].into(),
        std_message_size: vec![5.0, 60.0].into(),
        broadcast_chance: vec![0.1, 0.5, 0.9].into(),
        path_loss_exp: linspace(2.8, 4.2, 5),
        gateway_count: fixed(1),
        gateways_move: fixed(false),
        emergency_time_coef: vec![None, Some(0.5), Some(1.0), Some(1.2)].into(),
        with_fading: flip(),
    };

    println!("Wondering Squares: {}", params.len());

    loop {
        let params::WonderingRandomSquareData {
            side_len,
            node_count,
            message_count,
            mean_message_size,
            std_message_size,
            broadcast_chance,
            path_loss_exp,
            timespan,
            gateway_count,
            gateways_move,
            movement_speed,
            emergency_time_coef,
            with_fading,
        } = params.values();

        let scenario = ScenarioIdentity::Generated {
            generator: WonderingRandomSquare {
                node_count,
                messaging: IndependentRandomMessaging {
                    message_count,
                    messaging_timespan: timespan,
                    mean_message_size,
                    std_message_size,
                    broadcast_chance,
                    gateway_priority: 0.0,
                },
                model: if with_fading {
                    PairWiseCaptureEffect::default()
                        .with_pathloss(
                            AdjustedFreeSpacePathLoss::new(path_loss_exp, 0.0.into()).into(),
                        )
                        .with_fading(Normal::new(0.0, 4.0).unwrap())
                        .into()
                } else {
                    PairWiseCaptureEffect::default()
                        .with_pathloss(
                            AdjustedFreeSpacePathLoss::new(path_loss_exp, 0.0.into()).into(),
                        )
                        .into()
                },
                gateway_count,
                gateways_move,
                positioning: WonderingNodes {
                    side_len,
                    wonder_speed: movement_speed,
                    movement_timespan: timespan * 2.0,
                },
                emergency_time: emergency_time_coef.map(|n| timespan * n),
            },
            seed: seeding_rng.random(),
        };

        output.push(scenario);

        if !params.next() {
            break;
        }
    }

    output
}

fn pathways_one(seeding_rng: &mut ChaCha12Rng) -> Vec<ScenarioIdentity> {
    let mut output = Vec::new();

    let mut params = params::PathwaysOneParams {
        nth_pathway_chance: fixed(vec![1.0, 0.3, 0.1, 0.02]),
        mean_movement_speed: fixed(1.35 * MPS),
        std_movement_speed: fixed(0.2 * MPS),
        side_len: vec![1.0 * KM, 5.0 * KM, 10.0 * KM, 15.0 * KM].into(),
        message_count: vec![50, 100, 1000].into(),
        messaging_timespan: vec![5.0 * MINS, 10.0 * MINS, 30.0 * MINS, 60.0 * MINS].into(),
        mean_message_size: vec![20.0, 160.0].into(),
        std_message_size: vec![5.0, 50.0].into(),
        broadcast_chance: vec![0.1, 0.5, 0.9].into(),
        path_loss_exp: linspace(3.0, 4.5, 5),

        isolated_points_count: vec![1, 5, 30].into(),
        people_count: vec![10, 30, 100].into(),
        passive_key_points: fixed(4),
        radio_key_points: fixed(4),
        gateway_key_points: vec![1, 2].into(),
        isolated_gateway_count: vec![0, 1, 2].into(),
        emergency_time_coef: vec![None, Some(0.5), Some(1.0), Some(1.2)].into(),
    };

    println!("Pathways One: {}", params.len());

    loop {
        let params::PathwaysOneData {
            passive_key_points,
            radio_key_points,
            gateway_key_points,
            isolated_points_count,
            isolated_gateway_count,
            people_count,
            nth_pathway_chance,
            mean_movement_speed,
            std_movement_speed,
            side_len,
            message_count,
            messaging_timespan,
            mean_message_size,
            std_message_size,
            broadcast_chance,
            path_loss_exp,
            emergency_time_coef,
        } = params.values();

        let scenario = ScenarioIdentity::Generated {
            generator: PathwaysOne {
                messaging: IndependentRandomMessaging {
                    message_count,
                    messaging_timespan,
                    mean_message_size,
                    std_message_size,
                    broadcast_chance,
                    gateway_priority: 0.0,
                },
                model: PairWiseCaptureEffect::default()
                    .with_pathloss(AdjustedFreeSpacePathLoss::new(path_loss_exp, 0.0.into()).into())
                    .with_fading(Normal::new(0.0, 4.0).unwrap())
                    .into(),
                passive_key_points,
                radio_key_points,
                gateway_key_points,
                isolated_points_count,
                isolated_gateway_count,
                people_count,
                positioning: PathwayMovement {
                    side_len,
                    mean_movement_speed,
                    std_movement_speed,
                    nth_pathway_chance,
                },
                emergency_time: emergency_time_coef.map(|n| messaging_timespan * n),
            },
            seed: seeding_rng.random(),
        };

        output.push(scenario);

        if !params.next() {
            break;
        }
    }

    output
}

fn linspace<T>(start: T, end: T, count: usize) -> ParamVec<T>
where
    T: Add<Output = T>
        + Mul<f64, Output = T>
        + Sub<Output = T>
        + Div<f64, Output = T>
        + PartialOrd
        + Copy,
{
    let diff = end - start;
    let delta = diff / (count - 1) as f64;

    (0..count).map(|n| start + delta * n as f64).collect()
}

fn range<T>(range: Range<T>, increment: T) -> ParamVec<T>
where
    T: Add<Output = T> + PartialOrd + Copy,
{
    let mut first = range.start;
    iter::from_fn(move || {
        let tmp = first;
        first = first + increment;
        range.contains(&tmp).then_some(tmp)
    })
    .collect()
}

fn fixed<T>(value: T) -> ParamVec<T> {
    vec![value].into()
}

fn flip() -> ParamVec<bool> {
    vec![false, true].into()
}

struct ParamVec<T> {
    data: Vec<T>,
    index: usize,
}

impl<T> ParamVec<T>
where
    T: Clone,
{
    pub fn current(&self) -> T {
        self.data[self.index].clone()
    }

    pub fn next(&mut self) -> bool {
        self.index += 1;

        if self.index >= self.data.len() {
            self.index = 0;
            false
        } else {
            true
        }
    }
}

impl<T> From<Vec<T>> for ParamVec<T> {
    fn from(value: Vec<T>) -> Self {
        Self {
            data: value,
            index: 0,
        }
    }
}

impl<T> FromIterator<T> for ParamVec<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let data: Vec<T> = iter.into_iter().collect();
        Self { data, index: 0 }
    }
}
