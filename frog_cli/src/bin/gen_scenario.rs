use std::path::PathBuf;

use clap::{Parser, command};
use frogcore::{
    scenario::{
        ScenarioIdentity,
        generation::{
            ScenarioGenerator::{self, RandomSquare},
            messaging::IndependentRandomMessaging,
            positioning::{IndependentPositionFrames, PathwayMovement},
        },
    },
    sim_file::{self, load_file},
    simulation::models::{AdjustedFreeSpacePathLoss, NoneDist, PairWiseCaptureEffect},
    units::{Dbf, METRES, MINS, MPS, Temperature},
};
use rand::Rng;
use rand_distr::Normal;

#[derive(Parser, Debug)]
#[command()]
struct Args {
    #[arg(short, long)]
    output: Option<PathBuf>,

    #[arg(long)]
    paths: bool,

    #[arg(long)]
    tree: bool,

    #[arg(long)]
    graph: bool,

    #[arg(long)]
    spatial: bool,

    #[arg(long)]
    seed: Option<u64>,

    /// Generate from an identity
    #[arg(long)]
    id: Option<PathBuf>,

    /// Generate as an identity
    #[arg(long)]
    asid: bool,

    /// Use JSON instead of rust messagepack
    #[arg(long)]
    json: bool,
}

fn main() {
    let args = Args::parse();
    let seed: u64 = args.seed.unwrap_or_else(|| rand::rng().random());
    let output_file = args.output.unwrap_or("sim_file.sim".into());
    let use_rmp = !args.json;

    let id_file: Option<ScenarioIdentity> = args.id.map(|x| load_file(x).unwrap());

    let sim = if let Some(scenario) = id_file {
        scenario.create()
    } else if args.spatial {
        ScenarioIdentity::Generated {
            generator: ScenarioGenerator::PsudoSpatialGraph {
                nodes: 15,
                n_connections: 4,
                messaging: IndependentRandomMessaging {
                    message_count: 20,
                    messaging_timespan: 10.0 * MINS,
                    mean_message_size: 160.0,
                    std_message_size: 60.0,
                    broadcast_chance: 0.3,
                    gateway_priority: 0.0,
                },
                directed: false,
            },
            seed,
        }
        .create()
    } else if args.graph {
        ScenarioIdentity::Generated {
            generator: ScenarioGenerator::RandomTilConnectedGraph {
                nodes: 12,
                messaging: IndependentRandomMessaging {
                    message_count: 20,
                    messaging_timespan: 10.0 * MINS,
                    mean_message_size: 160.0,
                    std_message_size: 60.0,
                    broadcast_chance: 0.3,
                    gateway_priority: 0.0,
                },
            },
            seed,
        }
        .create()
    } else if args.tree {
        ScenarioIdentity::Generated {
            generator: ScenarioGenerator::SimpleTreeGraph {
                nodes: 30,
                min_degree: 1,
                max_degree: 10,
                messaging: IndependentRandomMessaging {
                    message_count: 20,
                    messaging_timespan: 10.0 * MINS,
                    mean_message_size: 160.0,
                    std_message_size: 60.0,
                    broadcast_chance: 0.3,
                    gateway_priority: 0.0,
                },
            },
            seed,
        }
        .create()
    } else if args.paths {
        ScenarioIdentity::Generated {
            generator: ScenarioGenerator::PathwaysOne {
                positioning: PathwayMovement {
                    mean_movement_speed: 40.0 * MPS,
                    std_movement_speed: 10.0 * MPS,
                    side_len: 30000.0 * METRES,
                    nth_pathway_chance: vec![1.0, 0.3],
                },

                passive_key_points: 3,
                radio_key_points: 2,
                gateway_key_points: 1,
                isolated_gateway_count: 1,
                isolated_points_count: 5,
                people_count: 30,

                messaging: IndependentRandomMessaging {
                    message_count: 10,
                    messaging_timespan: 10.0 * MINS,
                    mean_message_size: 160.0,
                    std_message_size: 60.0,
                    broadcast_chance: 0.3,
                    gateway_priority: 0.0,
                },
                model: PairWiseCaptureEffect::new(
                    AdjustedFreeSpacePathLoss::new(3.5, Dbf::from_db_value(0.0)).into(),
                    Temperature::ROOM_TEMP,
                    NoneDist,
                )
                .into(),
                emergency_time: None,
            },
            seed,
        }
        .create()
    } else {
        ScenarioIdentity::Generated {
            generator: RandomSquare {
                positioning: IndependentPositionFrames {
                    side_len: 10000.0 * METRES,
                    position_count: 3,
                    movement_timespan: 15.0 * MINS,
                },
                node_count: 50,
                messaging: IndependentRandomMessaging {
                    broadcast_chance: 0.1,
                    message_count: 200,
                    messaging_timespan: 10.0 * MINS,
                    mean_message_size: 120.0,
                    std_message_size: 60.0,
                    gateway_priority: 0.0,
                },
                model: PairWiseCaptureEffect::default()
                    .with_pathloss(AdjustedFreeSpacePathLoss::new(3.5, 0.0.into()).into())
                    .with_fading(Normal::new(0.0, 4.0).unwrap())
                    .into(),
                gateway_count: 2,
                gateways_move: false,
            },
            seed,
        }
        .create()
    };

    if args.asid {
        sim_file::write_file(output_file, sim.identity, use_rmp).unwrap();
    } else {
        sim_file::write_file(output_file, sim, use_rmp).unwrap();
    }
}
