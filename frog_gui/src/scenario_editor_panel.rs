use egui::{Color32, ComboBox, DragValue, Frame, Modal, RichText, Widget};

use macroquad::prelude::*;
use frogcore::{
    node_location::{NodeLocation, Point, Points, Timepoint},
    scenario::{
        MovementIndicator, Scenario, ScenarioIdentity, ScenarioMessage, ScenarioNodeSettings,
    },
    simulation::models::PairWiseCaptureEffect,
    units::{DbPerLength, METRES, SECONDS, Temperature, Unit},
};

use super::Inspectable;
use crate::{convert_rect, scene::SceneData};

pub struct ScenarioEditorPanel {
    scene: SceneData,
    pub scenario: Scenario,
    inspect_target: Inspectable,
    delete_node_pending: Option<usize>,
    message_sender_filter: Option<usize>,
    message_target_filter: Option<usize>,
}

impl ScenarioEditorPanel {
    pub fn new(mut scenario: Scenario) -> ScenarioEditorPanel {
        let mut scene = SceneData::new();
        scenario.identity = ScenarioIdentity::Custom;
        scene.zoom_to_fit(&scenario.map.display_locations(0.0 * SECONDS));

        ScenarioEditorPanel {
            scene,
            scenario,
            inspect_target: Inspectable::Nothing,
            delete_node_pending: None,
            message_sender_filter: None,
            message_target_filter: None,
        }
    }
}

pub fn new_scenario_and_panel() -> ScenarioEditorPanel {
    ScenarioEditorPanel::new(Scenario {
        identity: ScenarioIdentity::Custom,
        map: NodeLocation::Points(Points::new(vec![Timepoint {
            time: 0.0 * SECONDS,
            node_points: vec![Point {
                x: 0.0 * METRES,
                y: 0.0 * METRES,
            }],
        }])),
        model: PairWiseCaptureEffect::default().into(),
        messages: vec![],
        settings: vec![ScenarioNodeSettings::default()],
    })
}

impl Widget for &mut ScenarioEditorPanel {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let item_background = Color32::from_hex("#212121").unwrap();

        let Scenario {
            identity: _,
            map,
            model,
            messages,
            settings,
        } = &mut self.scenario;

        let map = match map {
            NodeLocation::Points(points) if points.data.len() == 1 => {
                &mut points.data[0].node_points
            }
            _ => {
                ui.label("Graphs and Points with movement are not yet supported");
                ui.label("Run the scenario from the top bar.");
                return ui.response();
            }
        };

        if let Some(delete_id) = self.delete_node_pending {
            let modal = Modal::new("Delete Node Modal".into()).show(ui.ctx(), |ui| {
                ui.heading(format!("Delete Node {delete_id}?"));
                ui.label("Assossiated messages will be deleted too.");
                ui.label("Nodes with higher ids will have their id decremented.");

                ui.horizontal_centered(|ui| {
                    if ui.button("Confirm").clicked() {
                        self.inspect_target = Inspectable::Nothing;

                        // Delete
                        map.remove(delete_id);
                        settings.remove(delete_id);
                        messages.retain(|x| x.sender != delete_id);
                        messages.retain(|x| {
                            x.targets.len() > 1 || *x.targets.first().unwrap() != delete_id
                        });

                        // Decrement
                        messages.iter_mut().for_each(|x| {
                            if x.sender > delete_id {
                                x.sender -= 1;
                            }

                            if x.targets.len() == 1 {
                                let target = x.targets.first_mut().unwrap();
                                if *target > delete_id {
                                    *target -= 1;
                                }
                            }
                        });

                        self.delete_node_pending = None;
                    };
                    if ui.button("Cancel").clicked() {
                        self.delete_node_pending = None;
                    }
                });
            });

            if modal.should_close() {
                self.delete_node_pending = None;
            }
        }

        egui::SidePanel::left("Scenario Editor Inspector").show_inside(ui, |ui| {
            node_setting_edit_panel(
                &mut self.inspect_target,
                settings,
                model,
                map,
                &mut self.delete_node_pending,
                ui,
            );
        });

        egui::SidePanel::right("Scenario Editor Message Panel").show_inside(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                message_editor_panel(
                    item_background,
                    messages,
                    &mut self.message_sender_filter,
                    &mut self.message_target_filter,
                    map,
                    ui,
                );
            });
        });

        let central_rect = egui::CentralPanel::default()
            .frame(Frame::NONE)
            .show_inside(ui, |ui| {
                self.scene.scene_egui(ui, true);
                ui.response()
            })
            .inner
            .rect;

        editor_scene(
            &mut self.inspect_target,
            &mut self.scene,
            convert_rect(central_rect),
            map,
            ui,
        );

        ui.response()
    }
}

fn editor_scene(
    inspect_target: &mut Inspectable,
    scene: &mut SceneData,
    scene_rect: Rect,
    map: &mut Vec<Point>,
    ui: &mut egui::Ui,
) {
    scene.camera_control(scene_rect);
    scene.select_and_reposition_interaction(inspect_target, map, scene_rect);

    set_camera(&scene.camera);
    scene.render_grid();
    scene.render_nodes(inspect_target, None, map, ui, scene_rect);
    scene.render_scale_indicator(ui, scene_rect);
}

fn message_editor_panel(
    item_background: Color32,
    messages: &mut Vec<ScenarioMessage>,
    sender_filter: &mut Option<usize>,
    target_filter: &mut Option<usize>,
    map: &mut Vec<Point>,
    ui: &mut egui::Ui,
) {
    ui.heading("Messages Editor");
    if ui.button("Add Message").clicked() {
        messages.insert(0, ScenarioMessage::new(0, vec![0], 1.0 * SECONDS, 160));
    }

    if ui.button("Sort By Time").clicked() {
        messages.sort_by(|first, second| {
            first
                .generate_time
                .inner()
                .total_cmp(&second.generate_time.inner())
        });
    }

    ui.separator();

    ui.label("Filters");

    let mut disable_sender_filter = false;
    let mut disable_target_filter = false;

    if let Some(ni) = sender_filter {
        ui.horizontal(|ui| {
            ui.label("Sender Filter: ");
            ui.add(DragValue::new(ni));
            if ui.button("Disable").clicked() {
                disable_sender_filter = true;
            }
        });
    } else {
        if ui.button("Filter Sender").clicked() {
            *sender_filter = Some(0);
        }
    }

    if let Some(ni) = target_filter {
        ui.horizontal(|ui| {
            ui.label("Target Filter: ");
            ui.add(DragValue::new(ni));
            if ui.button("Disable").clicked() {
                disable_target_filter = true;
            }
        });
    } else {
        if ui.button("Filter Target").clicked() {
            *target_filter = Some(0);
        }
    }

    if disable_sender_filter {
        *sender_filter = None;
    }
    if disable_target_filter {
        *target_filter = None;
    }

    ui.separator();

    let mut should_delete = None;

    for (
        index,
        ScenarioMessage {
            sender,
            targets,
            generate_time: send_time,
            size,
            ..
        },
    ) in messages.iter_mut().enumerate()
    {
        if let Some(ni) = sender_filter {
            if sender != ni {
                continue;
            }
        }
        if let Some(ni) = target_filter {
            if targets.contains(&ni) == false {
                continue;
            }
        }
        Frame::new()
            .inner_margin(3.0)
            .fill(item_background)
            .show(ui, |ui| {
                let mut time_float = send_time.seconds();
                ui.horizontal(|ui| {
                    ui.label("Sender: ");
                    ui.add(DragValue::new(sender).range(0..=map.len() - 1));
                });
                ui.horizontal(|ui| {
                    ui.label("Time:  ");
                    ui.add(
                        DragValue::new(&mut time_float)
                            .suffix(" s")
                            .range(0..=9999999),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Size: ");
                    ui.add(DragValue::new(size).suffix(" bytes").range(0..=255));
                });
                *send_time = time_float * SECONDS;

                let mut probably_broadcast = targets.len() > 1;

                if map.len() > 1 {
                    ui.horizontal(|ui| {
                        ui.label("Broadcast: ");
                        ui.checkbox(&mut probably_broadcast, "");
                    });

                    if probably_broadcast {
                        *targets = (0..map.len()).collect();
                        ui.label("");
                    } else {
                        let mut val = targets[0];
                        ui.horizontal(|ui| {
                            ui.label("Target: ");
                            ui.add(DragValue::new(&mut val));
                        });
                        *targets = vec![val];
                    }
                }

                ui.add_space(2.0);

                ui.with_layout(
                    egui::Layout::top_down_justified(egui::Align::Center),
                    |ui| {
                        if ui.button("Delete").clicked() {
                            should_delete = Some(index);
                        }
                    },
                );
            });
    }

    if let Some(delete_index) = should_delete {
        messages.remove(delete_index);
    }
}

fn node_setting_edit_panel(
    inspect_target: &mut Inspectable,
    settings: &mut Vec<ScenarioNodeSettings>,
    model: &mut frogcore::simulation::models::TransmissionModel,
    map: &mut Vec<Point>,
    modal_open: &mut Option<usize>,
    ui: &mut egui::Ui,
) {
    ui.heading("Node Editor");

    if ui.button("Add Node").clicked() {
        map.push(Point {
            x: 25.0 * METRES,
            y: 25.0 * METRES,
        });
        settings.push(ScenarioNodeSettings::default());
    }

    ui.separator();

    match *inspect_target {
        Inspectable::Node(id) => {
            ui.horizontal(|ui| {
                ui.label(format!("Editing Node ID {}", id));

                if *inspect_target != Inspectable::Nothing && ui.button("Deselect").clicked() {
                    *inspect_target = Inspectable::Nothing;
                }
            });
            inspect_node(&mut settings[id], &mut map[id], ui);
            ui.add_space(5.0);
            if ui.button("Delete Node").clicked() {
                *modal_open = Some(id);
            }
        }
        _ => {
            ui.label("No Node Selected");
        }
    }

    ui.separator();
    ui.add_space(30.0);
    ui.separator();

    // Simulation Settings
    ui.heading("Simulation Settings");

    ui.add_space(10.0);

    ui.label(RichText::new("Transmission Settings").underline());
    ui.add_space(5.0);

    use frogcore::simulation::models::*;
    let (path_loss, noise_temp) = match model {
        TransmissionModel::PairWiseNone(PairWiseCaptureEffect {
            path_loss,
            noise_temp,
            ..
        })
        | TransmissionModel::PairWiseNormal(PairWiseCaptureEffect {
            path_loss,
            noise_temp,
            ..
        })
        | TransmissionModel::PairWiseUniform(PairWiseCaptureEffect {
            path_loss,
            noise_temp,
            ..
        }) => (path_loss, noise_temp),
    };

    let pathloss_label = match path_loss {
        PathlossModel::NoPathloss(_) => "No Pathloss",
        PathlossModel::AdjustedFreeSpacePathLoss(_) => "Log Loss",
        PathlossModel::LinearPathLoss(_) => "Linear Loss",
    };
    {
        use PathlossModel::*;
        use frogcore::simulation::models;

        ui.horizontal(|ui| {
            ui.label("Pathloss Model");
            ComboBox::from_id_salt(603456)
                .selected_text(pathloss_label)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(matches!(path_loss, NoPathloss(_)), "No Pathloss")
                        .clicked()
                    {
                        *path_loss = NoPathloss(models::NoPathloss);
                    }
                    if ui
                        .selectable_label(
                            matches!(path_loss, AdjustedFreeSpacePathLoss(_)),
                            "Log Loss",
                        )
                        .clicked()
                    {
                        *path_loss = free_space_path_loss().into()
                    }
                    if ui
                        .selectable_label(matches!(path_loss, LinearPathLoss(_)), "Linear Loss")
                        .clicked()
                    {
                        *path_loss = LinearPathLoss(models::LinearPathLoss {
                            loss_rate: DbPerLength::from_db_per_metre(0.1),
                        })
                    }
                });
        });

        ui.indent(4523452, |ui| match path_loss {
            AdjustedFreeSpacePathLoss(free_space_path_loss) => {
                ui.horizontal(|ui| {
                    ui.label("Distance Exponent");
                    ui.add(DragValue::new(&mut free_space_path_loss.distance_exponent));
                });
            }
            LinearPathLoss(linear_path_loss) => {
                ui.horizontal(|ui| {
                    ui.label("Loss Rate");
                    let mut foo: f64 = linear_path_loss.loss_rate.into();
                    ui.add(DragValue::new(&mut foo).suffix(" dB / m"));
                    linear_path_loss.loss_rate = foo.into();
                });
            }
            _ => (),
        });

        ui.add_space(10.0);
        ui.label("Other Transmission Settings");

        ui.indent(56734567, |ui| {
            ui.horizontal(|ui| {
                ui.label("Noise Temprature");
                let mut val = noise_temp.celsius();

                ui.add(DragValue::new(&mut val).suffix(" °C"));
                *noise_temp = Temperature::from_celsius(val);
            });
        });

        ui.separator();
    }
}

fn inspect_node(current_node: &mut ScenarioNodeSettings, point: &mut Point, ui: &mut egui::Ui) {
    ui.add_space(5.0);
    ui.horizontal(|ui| {
        ui.label("Position");

        let (mut x, mut y) = (point.x.metres(), point.y.metres());

        ui.add(DragValue::new(&mut x).prefix("x:   ").suffix(" m"));
        ui.add(DragValue::new(&mut y).prefix("y:   ").suffix(" m"));

        *point = Point {
            x: x * METRES,
            y: y * METRES,
        };
    });

    ui.horizontal(|ui| {
        ui.label("Is Gateway: ");
        ui.checkbox(&mut current_node.is_gateway, "");
    });

    ui.horizontal(|ui| {
        ui.label("Movement Indicator: ");
        ComboBox::from_id_salt("Movement Indicator")
            .selected_text(format!("{:?}", current_node.movement_indicator))
            .show_ui(ui, |ui| {
                for value in MovementIndicator::VALUES {
                    ui.selectable_value(
                        &mut current_node.movement_indicator,
                        value,
                        format!("{:?}", value),
                    );
                }
            });
    });

    ui.add_space(5.0);
    ui.label(format!(
        "Bandwidth: {:.3} kHz",
        current_node.bandwidth.kHz()
    ));
    ui.label(format!("Carrier Band: {:?}", current_node.carrier_band));
    ui.label(format!(
        "Max Power: {:.3} dBm",
        current_node.max_power.dbm()
    ));
    ui.label(format!("Default SF: {}", current_node.sf));
}
