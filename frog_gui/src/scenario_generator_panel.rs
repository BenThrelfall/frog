use std::{cell::RefCell, sync::Arc};

use egui::{ComboBox, DragValue, Frame, RichText, Widget};
use macroquad::prelude::rand;
use frogcore::{
    scenario::{
        ScenarioIdentity,
        generation::{
            ScenarioGenerator,
            messaging::IndependentRandomMessaging,
            positioning::{IndependentPositionFrames, PathwayMovement, WonderingNodes},
        },
    },
    simulation::models::{
        AdjustedFreeSpacePathLoss, Normal, PairWiseCaptureEffect, adjusted_free_space_path_loss,
    },
    units::{KM, METRES, MINS, MPS},
};

use crate::{GlobalAction, GuiStore, components::UiExt};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GeneratorSelection {
    PsudoSpatialGraph,
    RandomSquare,
    WonderingRandomSquare,
    PathwaysOne,
    SimpleTreeGraph,
    RandomTilConnectedGraph,
}

const GENERATOR_LIST: [GeneratorSelection; 6] = [
    GeneratorSelection::PsudoSpatialGraph,
    GeneratorSelection::RandomSquare,
    GeneratorSelection::WonderingRandomSquare,
    GeneratorSelection::PathwaysOne,
    GeneratorSelection::SimpleTreeGraph,
    GeneratorSelection::RandomTilConnectedGraph,
];

pub struct ScenarioGeneratorPanel {
    seed: u64,
    generator: ScenarioGenerator,
    generator_selection: GeneratorSelection,
    store: Arc<RefCell<GuiStore>>,

    // Random Placement
    rp_node_count: usize,
    rp_side_len: f64,

    // Pathways
    paths_node_count: usize,
    paths_side_len: f64,

    // Graph
    graph_node_count: usize,
    graph_min_degree: usize,
}

impl ScenarioGeneratorPanel {
    pub fn new(store: Arc<RefCell<GuiStore>>) -> Self {
        ScenarioGeneratorPanel {
            generator: ScenarioGenerator::RandomSquare {
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
            seed: 1,
            store,
            generator_selection: GeneratorSelection::RandomSquare,
            rp_node_count: 10,
            rp_side_len: 5000.,
            paths_node_count: 20,
            paths_side_len: 5000.,
            graph_node_count: 10,
            graph_min_degree: 2,
        }
    }
}

impl Widget for &mut ScenarioGeneratorPanel {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        Frame::new().outer_margin(20).show(ui, |ui| {
            ui.label(RichText::new("Simple Generators").heading().size(32.));

            ui.columns_const(|[col1, col2, col3]| {
                col1.heading("Random Placement");
                col1.label("Radio nodes are scattered randomly in a square area.");

                col1.add_space(5.);
                col1.numeric_edit("Node Count: ", &mut self.rp_node_count);
                col1.unit_edit("Area Side Length", &mut self.rp_side_len, "m");

                if col1.button("Generate").clicked() {
                    self.store.borrow_mut().global_action = GlobalAction::SetScenario(
                        ScenarioIdentity::Generated {
                            generator: ScenarioGenerator::RandomSquare {
                                node_count: self.rp_node_count,
                                gateway_count: 0,
                                gateways_move: false,
                                positioning: IndependentPositionFrames {
                                    side_len: self.rp_side_len * METRES,
                                    position_count: 1,
                                    movement_timespan: 1.0 * MINS,
                                },
                                messaging: default_messaging(),
                                model: PairWiseCaptureEffect::default()
                                    .with_pathloss(adjusted_free_space_path_loss(3.7).into())
                                    .into(),
                            },
                            seed: rand::rand() as u64,
                        }
                        .create(),
                    )
                }

                col2.heading("Pathways");
                col2.label("Keypoints are placed randomly in a square area.");
                col2.label("Radio nodes travel along straight paths between the keypoints.");
                col2.add_space(5.);

                col2.numeric_edit("Node Count: ", &mut self.paths_node_count);
                col2.unit_edit("Area Side Length", &mut self.paths_side_len, "m");

                if col2.button("Generate").clicked() {
                    self.store.borrow_mut().global_action = GlobalAction::RunScenario(
                        ScenarioIdentity::Generated {
                            generator: ScenarioGenerator::PathwaysOne {
                                passive_key_points: 8,
                                radio_key_points: 0,
                                gateway_key_points: 0,
                                isolated_points_count: 0,
                                isolated_gateway_count: 0,
                                people_count: self.paths_node_count,
                                emergency_time: None,
                                messaging: default_messaging(),
                                positioning: PathwayMovement {
                                    side_len: self.paths_side_len * METRES,
                                    mean_movement_speed: 3.0 * MPS,
                                    std_movement_speed: 2.0 * MPS,
                                    nth_pathway_chance: vec![1.0, 0.5, 0.1],
                                },
                                model: PairWiseCaptureEffect::default()
                                    .with_pathloss(adjusted_free_space_path_loss(3.5).into())
                                    .into(),
                            },
                            seed: rand::rand() as u64,
                        }
                        .create(),
                    )
                }

                col3.heading("Graph");
                col3.label("A graph where radio nodes are in range of each other if and only if an edge exists between them.");

                col3.add_space(5.);
                col3.numeric_edit("Node Count: ", &mut self.graph_node_count);
                col3.numeric_edit("Minimum Degree: ", &mut self.graph_min_degree);

                if col3.button("Generate").clicked() {
                    self.store.borrow_mut().global_action = GlobalAction::RunScenario(
                        ScenarioIdentity::Generated {
                            generator: ScenarioGenerator::PsudoSpatialGraph {
                                nodes: self.graph_node_count,
                                n_connections: self.graph_min_degree,
                                messaging: default_messaging(),
                                directed: false,
                            },
                            seed: rand::rand() as u64,
                        }
                        .create(),
                    )
                }
            });

            ui.add_space(200.0);
            ui.separator();

            ui.label(RichText::new("Custom Generator").heading().size(32.));

            ui.horizontal(|ui| {
                if ui.button("Generate").clicked() {
                    self.store.borrow_mut().global_action = GlobalAction::SetScenario(
                        ScenarioIdentity::Generated {
                            generator: self.generator.clone(),
                            seed: self.seed,
                        }
                        .create(),
                    )
                }

                ui.label("with seed: ");

                ui.add(DragValue::new(&mut self.seed));
            });
            ui.heading("Generator Type");

            let prev = self.generator_selection;
            ComboBox::from_label("Generator")
                .selected_text(format!("{:?}", self.generator_selection))
                .show_ui(ui, |ui| {
                    for model in GENERATOR_LIST {
                        ui.selectable_value(
                            &mut self.generator_selection,
                            model,
                            format!("{:?}", model),
                        );
                    }
                });

            if self.generator_selection != prev {
                self.generator = self.generator_selection.into();
            }

            let mut value = serde_inspector::to_value(&self.generator).unwrap();
            ui.heading("Settings");
            serde_inspector::any_editor(12345, &mut value, ui);

            self.generator = value.deserialize_into().unwrap();
        });

        ui.response()
    }
}

impl From<GeneratorSelection> for ScenarioGenerator {
    fn from(value: GeneratorSelection) -> Self {
        match value {
            GeneratorSelection::PsudoSpatialGraph => ScenarioGenerator::PsudoSpatialGraph {
                nodes: 10,
                n_connections: 3,
                messaging: default_messaging(),
                directed: false,
            },
            GeneratorSelection::RandomSquare => ScenarioGenerator::RandomSquare {
                node_count: 10,
                gateway_count: 0,
                gateways_move: false,
                positioning: IndependentPositionFrames {
                    side_len: 5.0 * KM,
                    position_count: 1,
                    movement_timespan: 1.0 * MINS,
                },
                messaging: default_messaging(),
                model: PairWiseCaptureEffect::default().into(),
            },
            GeneratorSelection::WonderingRandomSquare => ScenarioGenerator::WonderingRandomSquare {
                node_count: 10,
                gateway_count: 0,
                gateways_move: false,
                emergency_time: None,
                positioning: WonderingNodes {
                    side_len: 5.0 * KM,
                    movement_timespan: 1.0 * MINS,
                    wonder_speed: 1.0 * MPS,
                },
                messaging: default_messaging(),
                model: PairWiseCaptureEffect::default().into(),
            },
            GeneratorSelection::PathwaysOne => ScenarioGenerator::PathwaysOne {
                passive_key_points: 5,
                radio_key_points: 3,
                gateway_key_points: 0,
                isolated_points_count: 4,
                isolated_gateway_count: 0,
                people_count: 20,
                emergency_time: None,
                messaging: default_messaging(),
                positioning: PathwayMovement {
                    side_len: 5.0 * KM,
                    mean_movement_speed: 2.0 * MPS,
                    std_movement_speed: 1.0 * MPS,
                    nth_pathway_chance: vec![1.0, 0.5, 0.1],
                },
                model: PairWiseCaptureEffect::default().into(),
            },
            GeneratorSelection::SimpleTreeGraph => ScenarioGenerator::SimpleTreeGraph {
                nodes: 10,
                min_degree: 1,
                max_degree: 5,
                messaging: default_messaging(),
            },
            GeneratorSelection::RandomTilConnectedGraph => {
                ScenarioGenerator::RandomTilConnectedGraph {
                    nodes: 10,
                    messaging: default_messaging(),
                }
            }
        }
    }
}

fn default_messaging() -> IndependentRandomMessaging {
    IndependentRandomMessaging {
        message_count: 30,
        messaging_timespan: 5.0 * MINS,
        mean_message_size: 120.0,
        std_message_size: 40.0,
        broadcast_chance: 0.1,
        gateway_priority: 0.0,
    }
}
