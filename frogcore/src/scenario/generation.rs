pub mod messaging;
pub mod positioning;

use std::collections::{HashSet, VecDeque};

use messaging::IndependentRandomMessaging;
use positioning::{IndependentPositionFrames, PathwayMovement, WonderingNodes, pos_random_square};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha12Rng;
use serde::{Deserialize, Serialize};

use crate::{
    node_location::{Edge, Graph, NodeLocation, Points},
    scenario::{MessageMarker, MovementIndicator, ScenarioMessage, ScenarioNodeSettings},
    scenario::{Scenario, ScenarioIdentity},
    simulation::models::{PairWiseCaptureEffect, TransmissionModel},
    units::*,
    utility::n_min,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ScenarioGenerator {
    PsudoSpatialGraph {
        nodes: usize,
        n_connections: usize,
        messaging: IndependentRandomMessaging,
        directed: bool,
    },
    RandomSquare {
        /// Number of nodes that are not gateways.
        /// `total_nodes = node_count + gateway_count`
        node_count: usize,

        /// Number of gateways.
        /// `total_nodes = node_count + gateway_count`
        gateway_count: usize,

        /// Indicates if gateways should move or not.
        /// No effect if `position_count = 1`
        gateways_move: bool,

        positioning: IndependentPositionFrames,
        messaging: IndependentRandomMessaging,

        model: TransmissionModel,
    },
    WonderingRandomSquare {
        /// Number of nodes that are not gateways.
        /// `total_nodes = node_count + gateway_count`
        node_count: usize,

        /// Number of gateways.
        /// `total_nodes = node_count + gateway_count`
        gateway_count: usize,

        /// Indicates if gateways should move or not.
        /// No effect if `position_count = 1`
        gateways_move: bool,

        /// If set, an emergency will occur at this time
        emergency_time: Option<Time>,

        positioning: WonderingNodes,
        messaging: IndependentRandomMessaging,

        model: TransmissionModel,
    },
    PathwaysOne {
        /// A key point people will move between
        /// without a radio
        passive_key_points: usize,

        /// A key point people will move between
        /// with a radio but not a gateway
        radio_key_points: usize,

        /// A key point people will move between
        /// with a radio and a gateway
        gateway_key_points: usize,

        /// A statonary point ignored by people
        /// with a radio and without a gateway
        isolated_points_count: usize,

        /// A statonary point ignored by people
        /// with a radio and a gateway
        isolated_gateway_count: usize,

        /// Number of people each with a radio
        /// they will move between the keypoints
        people_count: usize,

        /// If set, an emergency will occur at this time
        emergency_time: Option<Time>,

        messaging: IndependentRandomMessaging,
        positioning: PathwayMovement,

        model: TransmissionModel,
    },
    SimpleTreeGraph {
        nodes: usize,
        min_degree: usize,
        max_degree: usize,
        messaging: IndependentRandomMessaging,
    },
    RandomTilConnectedGraph {
        nodes: usize,
        messaging: IndependentRandomMessaging,
    },
}

impl ScenarioGenerator {
    pub fn generate_from_seed(&self, seed: u64) -> Scenario {
        let rng = ChaCha12Rng::seed_from_u64(seed);
        let output = self.generate(rng);
        output
    }

    pub fn generate(&self, mut rng: ChaCha12Rng) -> Scenario {
        match self.clone() {
            ScenarioGenerator::WonderingRandomSquare {
                node_count,
                gateway_count,
                gateways_move,
                positioning,
                messaging,
                model,
                emergency_time,
            } => {
                let map = if gateways_move {
                    positioning.generate(node_count + gateway_count, 0, &mut rng)
                } else {
                    positioning.generate(node_count, gateway_count, &mut rng)
                };

                let map = NodeLocation::Points(Points::new(map));

                let settings: Vec<_> = (0..node_count + gateway_count)
                    .map(|index| {
                        let mut val = ScenarioNodeSettings::default();
                        if index < node_count {
                            val.movement_indicator = MovementIndicator::Mobile;
                        } else {
                            let mut val = ScenarioNodeSettings::default();
                            val.is_gateway = true;

                            val.movement_indicator = if gateways_move {
                                MovementIndicator::Mobile
                            } else {
                                MovementIndicator::Stationary
                            };
                        }
                        val
                    })
                    .collect();

                let mut messages = messaging.generate(&settings, &mut rng);

                if let Some(time) = emergency_time {
                    let emergency_node = rng.random_range(0..node_count);
                    messages.push(
                        ScenarioMessage::new(
                            emergency_node,
                            (0..settings.len())
                                .filter(|x| *x != emergency_node)
                                .collect(),
                            time,
                            32,
                        )
                        .with_marker(MessageMarker::Emergency)
                        .with_repeats(30, 10.0 * SECONDS),
                    );
                }

                Scenario {
                    identity: ScenarioIdentity::Custom,
                    map,
                    model,
                    messages,
                    settings,
                }
            }
            ScenarioGenerator::RandomSquare {
                node_count,
                messaging,
                gateway_count,
                gateways_move,
                positioning,
                model,
            } => {
                let map = if gateways_move {
                    positioning.generate(node_count + gateway_count, 0, &mut rng)
                } else {
                    positioning.generate(node_count, gateway_count, &mut rng)
                };

                let map = NodeLocation::Points(Points::new(map));

                let settings: Vec<_> = (0..node_count + gateway_count)
                    .map(|index| {
                        let mut val = ScenarioNodeSettings::default();
                        if index < node_count {
                            val.movement_indicator = MovementIndicator::Mobile;
                        } else {
                            let mut val = ScenarioNodeSettings::default();
                            val.is_gateway = true;

                            val.movement_indicator = if gateways_move {
                                MovementIndicator::Mobile
                            } else {
                                MovementIndicator::Stationary
                            };
                        }
                        val
                    })
                    .collect();

                let messages = messaging.generate(&settings, &mut rng);

                Scenario {
                    identity: ScenarioIdentity::Custom,
                    map,
                    model,
                    messages,
                    settings,
                }
            }
            ScenarioGenerator::PathwaysOne {
                passive_key_points,
                radio_key_points,
                gateway_key_points,
                isolated_points_count,
                isolated_gateway_count,
                people_count,
                messaging,
                positioning,
                model,
                emergency_time,
            } => {
                // [isolated_points, active_key_points, people]
                let map = positioning.generate(
                    isolated_points_count + isolated_gateway_count,
                    radio_key_points + gateway_key_points,
                    passive_key_points,
                    people_count,
                    messaging.messaging_timespan * 2.0,
                    &mut rng,
                );

                let map = NodeLocation::Points(Points::new(map));

                let settings: Vec<_> = (0..isolated_points_count)
                    .map(|_| ScenarioNodeSettings::default())
                    .chain(
                        (0..isolated_gateway_count)
                            .map(|_| ScenarioNodeSettings::default().as_gateway()),
                    )
                    .chain((0..radio_key_points).map(|_| ScenarioNodeSettings::default()))
                    .chain(
                        (0..gateway_key_points)
                            .map(|_| ScenarioNodeSettings::default().as_gateway()),
                    )
                    .chain((0..people_count).map(|_| {
                        ScenarioNodeSettings::default()
                            .with_movement_indicator(MovementIndicator::Mobile)
                    }))
                    .collect();

                let mut messages = messaging.generate(&settings, &mut rng);

                if let Some(time) = emergency_time {
                    let emergency_node = rng.random_range(0..isolated_points_count);
                    messages.push(
                        ScenarioMessage::new(
                            emergency_node,
                            (0..settings.len())
                                .filter(|x| *x != emergency_node)
                                .collect(),
                            time,
                            32,
                        )
                        .with_marker(MessageMarker::Emergency)
                        .with_repeats(30, 10.0 * SECONDS),
                    );
                }

                Scenario {
                    identity: ScenarioIdentity::Custom,
                    map,
                    model,
                    messages,
                    settings,
                }
            }
            ScenarioGenerator::SimpleTreeGraph {
                nodes,
                min_degree,
                max_degree,
                messaging,
            } => {
                let mut expand = VecDeque::new();
                let mut graph = Vec::new();

                assert!(min_degree > 0);

                graph.push(Vec::new());
                expand.push_back(0);

                while graph.len() < nodes {
                    let this = expand.pop_front().unwrap_or(graph.len() - 1);

                    let add_nodes = rng.random_range(min_degree - 1..max_degree);
                    let add_nodes = add_nodes.min(nodes - graph.len());

                    for _i in 0..add_nodes {
                        let new_index = graph.len();
                        expand.push_back(new_index);
                        graph[this].push(Edge {
                            to: new_index,
                            weight: 100.0 * METRES,
                        });

                        graph.push(vec![Edge {
                            to: this,
                            weight: 100.0 * METRES,
                        }]);
                    }
                }

                let model = PairWiseCaptureEffect::default().into();

                let settings = vec![ScenarioNodeSettings::default(); nodes];
                let messages = messaging.generate(&settings, &mut rng);

                assert_eq!(settings.len(), graph.len());

                Scenario {
                    identity: ScenarioIdentity::Custom,
                    map: NodeLocation::Graph(Graph::new(graph)),
                    model,
                    messages,
                    settings,
                }
            }
            ScenarioGenerator::RandomTilConnectedGraph { nodes, messaging } => {
                let mut graph = vec![Vec::new(); nodes];

                while !graph_is_connected(&graph) {
                    let node_a = rng.random_range(0..nodes);
                    let node_b = loop {
                        let val = rng.random_range(0..nodes);
                        if val != node_a {
                            break val;
                        }
                    };

                    if graph[node_a].iter().find(|x| x.to == node_b).is_none() {
                        graph[node_a].push(Edge {
                            to: node_b,
                            weight: 100.0 * METRES,
                        });
                        graph[node_b].push(Edge {
                            to: node_a,
                            weight: 100.0 * METRES,
                        });
                    }
                }

                let model = PairWiseCaptureEffect::default().into();

                let settings = vec![ScenarioNodeSettings::default(); nodes];
                let messages = messaging.generate(&settings, &mut rng);

                Scenario {
                    identity: ScenarioIdentity::Custom,
                    map: NodeLocation::Graph(Graph::new(graph)),
                    model,
                    messages,
                    settings,
                }
            }
            ScenarioGenerator::PsudoSpatialGraph {
                nodes,
                n_connections,
                messaging,
                directed,
            } => {
                let points = pos_random_square(nodes, 1000.0 * METRES, &mut rng);

                let mut graph: Vec<Vec<Edge>> = vec![Vec::new(); nodes];

                for (n, point) in points.iter().enumerate() {
                    let distances: Vec<Length> =
                        points.iter().map(|other| (*point - *other).mag()).collect();

                    let closests = n_min(&distances, n_connections + 1);

                    closests.iter().skip(1).for_each(|i| {
                        if graph[n].iter().find(|x| x.to == *i).is_none() {
                            graph[n].push(Edge {
                                to: *i,
                                weight: 100.0 * METRES,
                            });
                            if !directed {
                                graph[*i].push(Edge {
                                    to: n,
                                    weight: 100.0 * METRES,
                                });
                            }
                        }
                    });
                }

                let settings = vec![ScenarioNodeSettings::default(); nodes];
                let messages = messaging.generate(&settings, &mut rng);

                let model = PairWiseCaptureEffect::default().into();

                Scenario {
                    identity: ScenarioIdentity::Custom,
                    map: NodeLocation::Graph(Graph::new(graph)),
                    model,
                    messages,
                    settings,
                }
            }
        }
    }
}

fn graph_is_connected(graph: &Vec<Vec<Edge>>) -> bool {
    let mut visited = HashSet::new();
    let mut expand = VecDeque::new();

    expand.push_back(0);

    while !expand.is_empty() {
        let node = expand.pop_front().unwrap();
        visited.insert(node);

        for adj in &graph[node] {
            if !visited.contains(&adj.to) {
                expand.push_back(adj.to);
            }
        }
    }

    visited.len() == graph.len()
}
