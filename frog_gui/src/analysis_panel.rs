use egui::{
    Align, Color32, CornerRadius, DragValue, Frame, Label, Layout, Pos2, RichText, ScrollArea,
    Stroke, Widget, style::WidgetVisuals,
};

use std::collections::{HashMap, HashSet};

use macroquad::prelude::*;

use frogcore::{
    analysis::{CompleteAnalysis, TransmissionGraph, WantedMessage, create_transmission_graphs},
    node::NodeModel,
    node_location::NodeLocation,
    scenario::{Scenario, ScenarioNodeSettings},
    sim_file::SimOutput,
    simulation::{
        LiveSimulation, MessageContent,
        data_structs::{LogItem, Transmission},
        run_simulation,
    },
    units::{METRES, Time},
};

use crate::scene::{SceneData, point_to_vec};
use crate::{Inspectable, convert_rect, get_event_window, short_content};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InspectorTab {
    Overview,
    State,
}

pub struct AnalysisPanel {
    scene: SceneData,
    node_locations: NodeLocation,
    node_settings: Vec<ScenarioNodeSettings>,
    node_events: Vec<Vec<LogItem>>,
    wanted_messages: Vec<Vec<WantedMessage>>,
    received_messages: Vec<Vec<usize>>,
    reception_rate: Vec<f64>,
    sim_events: Vec<LogItem>,
    transmission_graphs: HashMap<u32, TransmissionGraph>,
    transmissions: Vec<Transmission>,
    inspect_target: Inspectable,
    current_time: f64,
    prev_time: f64,
    end_time: f64,
    playing: bool,
    play_timescale: f64,
    play_offset: f64,
    play_time_offset: f64,
    used_seed: u64,
    used_model: String,
    inspector_tabs: InspectorTab,
    use_inspector_text_mode: bool,
    live_sim: Option<LiveSimulation>,
}

impl AnalysisPanel {
    pub fn new(scenario: Scenario, results: SimOutput) -> AnalysisPanel {
        let CompleteAnalysis {
            node_settings,
            node_events,
            sim_events,
            transmissions,
            end_time,
            reception_analysis:
                frogcore::analysis::ReceptionAnalysis {
                    wanted_messages,
                    received_messages,
                    reception_rate,
                    ..
                },
            total_airtime: _,
            complete_identity,
            ..
        } = CompleteAnalysis::new(results, scenario.clone());

        let node_locations = scenario.map;
        let transmission_graphs = create_transmission_graphs(sim_events.clone());

        let mut scene = SceneData::new();
        scene.zoom_to_fit(&node_locations.display_locations(Time::from_seconds(0.0)));

        AnalysisPanel {
            node_locations,
            node_settings,
            node_events,
            sim_events,
            transmission_graphs,
            transmissions,
            end_time,
            wanted_messages,
            received_messages,
            reception_rate,
            used_seed: complete_identity.simulation_seed,
            used_model: complete_identity.model_id,
            scene,
            inspect_target: Inspectable::Nothing,
            current_time: 0.0,
            prev_time: 0.0,
            playing: false,
            play_timescale: 1.0,
            play_offset: 0.0,
            play_time_offset: 0.0,
            inspector_tabs: InspectorTab::Overview,
            use_inspector_text_mode: false,
            live_sim: None,
        }
    }

    pub fn from_scenario(scenario: Scenario, model: NodeModel) -> AnalysisPanel {
        let live = LiveSimulation::new(12345, scenario.clone(), model.clone(), true);
        let sim_output = run_simulation(12345, scenario.clone(), model, true);

        let mut out = AnalysisPanel::new(scenario, sim_output);

        out.live_sim = Some(live);

        out
    }

    fn event_ui(events: &Vec<LogItem>, ui: &mut egui::Ui, time: Time) {
        let mut in_future = false;

        get_event_window(events, time).for_each(|x| {
            if !in_future && x.time > time {
                in_future = true;
                ui.separator();
                ui.add(Label::new(RichText::new("Upcoming").underline()));
            }
            ui.label(format!("<{:.3}> {}", x.time, x.content));
        });
    }
}

impl Widget for &mut AnalysisPanel {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let node_locations = self
            .node_locations
            .display_locations(Time::from_seconds(self.current_time));

        let item_background = Color32::from_hex("#212121").unwrap();
        let main_red = Color32::from_hex("#9b0d0d").unwrap();

        egui::TopBottomPanel::top("timeline").show_inside(ui, |ui| {
            self.analysis_timeline_panel(item_background, main_red, ui);
        });

        egui::SidePanel::left("inspector")
            .max_width(500.0)
            .min_width(350.0)
            .show_inside(ui, |ui| {
                self.analysis_inspector_panel(&node_locations, item_background, ui)
            });

        egui::SidePanel::right("right_panel")
            .min_width(285.0)
            .show_inside(ui, |ui| self.analysis_events_panel(item_background, ui));

        egui::TopBottomPanel::bottom("transmission_timeline")
            .min_height(150.0)
            .show_inside(ui, |ui| {
                self.analysis_transmission_timeline(main_red, ui);
            });

        let central_rect = egui::CentralPanel::default()
            .frame(Frame::NONE)
            .show_inside(ui, |ui| {
                self.scene.scene_egui(ui, false);
                ui.response()
            })
            .inner
            .rect;

        self.analysis_scene_panel(node_locations, ui, convert_rect(central_rect));

        ui.response()
    }
}

impl AnalysisPanel {
    fn analysis_scene_panel(
        &mut self,
        node_locations: Vec<frogcore::node_location::Point>,
        ui: &mut egui::Ui,
        scene_rect: Rect,
    ) {
        self.scene.camera_control(scene_rect);
        self.scene
            .select_interaction(&mut self.inspect_target, &node_locations, scene_rect);

        set_camera(&self.scene.camera);
        self.scene.render_grid();
        self.scene.render_scale_indicator(ui, scene_rect);
        let node_size = self.scene.node_size();

        let mut senders = HashSet::new();

        let line_base_size = 2. / self.scene.zoom_level;

        match self.node_locations {
            NodeLocation::Graph(_) => {
                for (i, point) in node_locations.iter().enumerate() {
                    for index in self.node_locations.get_adj(i) {
                        let other = node_locations[index];

                        let start = point_to_vec(*point);
                        let end = point_to_vec(other);
                        draw_line(start.x, start.y, end.x, end.y, 3.0 * line_base_size, BLACK);

                        let marker = point_to_vec(
                            *point + (other - *point).clamp_mag(node_size as f64 * 1.2 * METRES),
                        );
                        draw_circle(marker.x, marker.y, 5.0 * line_base_size, BLACK);
                    }
                }
            }
            _ => (),
        }

        for (id, web) in {
            self.transmissions.iter().filter(|x| {
                x.start_time <= self.current_time.into() && x.end_time >= self.current_time.into()
            })
        }
        .map(|x| x.id)
        .map(|id| (id, &self.transmission_graphs[&id]))
        {
            let origin = point_to_vec(node_locations[web.origin]);
            senders.insert(web.origin);

            for target in web.targets.iter().copied() {
                let target_pos = point_to_vec(node_locations[target]);

                let line_colour = if Inspectable::Transmission(id) == self.inspect_target {
                    GREEN
                } else {
                    ORANGE
                };

                draw_line(
                    origin.x,
                    origin.y,
                    target_pos.x,
                    target_pos.y,
                    3.0 * line_base_size,
                    line_colour,
                );
            }
        }

        self.scene.render_nodes(
            &mut self.inspect_target,
            Some(&senders),
            &node_locations,
            ui,
            scene_rect,
        );
    }

    fn analysis_transmission_timeline(&mut self, main_red: Color32, ui: &mut egui::Ui) {
        let timespan = 10.0;
        let timeline_trans = self.transmissions.iter().filter(|x| {
            x.end_time.seconds() > self.current_time - timespan
                && x.start_time.seconds() < self.current_time + timespan
        });

        let size_adjust = ui.max_rect().width() / (timespan as f32 * 2.0);
        let offset = ui.next_widget_position().to_vec2();

        ui.painter().rect_filled(
            egui::Rect {
                min: Pos2::new(ui.max_rect().width() / 2.0 - 1.0, 0.0),
                max: Pos2::new(ui.max_rect().width() / 2.0 + 1.0, 150.0),
            }
            .translate(offset),
            0.0,
            Color32::BLACK,
        );

        for transmission in timeline_trans {
            let row = (transmission.id % 5) as f32;
            let pos_rect = egui::Rect {
                min: Pos2::new(
                    (transmission.start_time.seconds() + timespan - self.current_time) as f32
                        * size_adjust,
                    row * 30.0,
                ),
                max: Pos2::new(
                    (transmission.end_time.seconds() + timespan - self.current_time) as f32
                        * size_adjust,
                    (row + 0.9) * 30.0,
                ),
            }
            .translate(offset);

            ui.painter().rect_filled(pos_rect, 0.0, main_red);

            if ui
                .put(pos_rect, Label::new(transmission.id.to_string()))
                .clicked()
            {
                self.inspect_target = Inspectable::Transmission(transmission.id);
            }
        }
    }

    fn analysis_events_panel(
        &mut self,
        item_background: Color32,
        ui: &mut egui::Ui,
    ) -> egui::scroll_area::ScrollAreaOutput<()> {
        let active_transmissions = {
            self.transmissions.iter().filter(|x| {
                x.start_time <= self.current_time.into() && x.end_time >= self.current_time.into()
            })
        };

        let upcoming_transmissions = {
            self.transmissions
                .iter()
                .skip_while(|x| x.start_time <= self.current_time.into())
                .take(3)
        };

        ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Sim Events");
            AnalysisPanel::event_ui(&self.sim_events, ui, self.current_time.into());

            ui.separator();

            ui.heading("Transmissions");
            ui.add(Label::new(RichText::new("Active").underline().weak()));
            active_transmissions.for_each(|x| {
                Frame::new()
                    .fill(item_background)
                    .inner_margin(5.0)
                    .show(ui, |ui| {
                        ui.label(format!(
                        "Id: {}    Sender: {}    Content: {} \nAirtime: {:.3}s -> {:.3}s  ({:.3}s)",
                        x.id,
                        x.transmitter_id,
                        short_content(&x.message_content),
                        x.start_time,
                        x.end_time,
                        x.airtime()
                    ))
                    .clicked()
                    .then(|| self.inspect_target = Inspectable::Transmission(x.id));
                    });
            });

            ui.separator();
            ui.add(Label::new(RichText::new("Upcoming").underline().weak()));
            upcoming_transmissions.for_each(|x| {
                Frame::new()
                    .fill(item_background)
                    .inner_margin(5.0)
                    .show(ui, |ui| {
                        ui.label(format!(
                        "Id: {}    Sender: {}    Content: {} \nAirtime: {:.3}s -> {:.3}s  ({:.3}s)",
                        x.id,
                        x.transmitter_id,
                        short_content(&x.message_content),
                        x.start_time,
                        x.end_time,
                        x.airtime()
                    ))
                    .clicked()
                    .then(|| self.inspect_target = Inspectable::Transmission(x.id));
                    });
            });
        })
    }

    fn analysis_inspector_panel(
        &mut self,
        node_locations: &Vec<frogcore::node_location::Point>,
        item_background: Color32,
        ui: &mut egui::Ui,
    ) -> egui::scroll_area::ScrollAreaOutput<()> {
        macro_rules! set_time {
            ($new_time: expr) => {
                let new_time = $new_time.into();
                self.prev_time = self.current_time;
                self.current_time = new_time;
            };
        }

        ui.horizontal(|ui| {
            if ui
                .selectable_label(self.inspector_tabs == InspectorTab::Overview, "Overview")
                .clicked()
            {
                self.inspector_tabs = InspectorTab::Overview;
            }
            if ui
                .selectable_label(self.inspector_tabs == InspectorTab::State, "State")
                .clicked()
            {
                self.inspector_tabs = InspectorTab::State;
            }
            ui.add_space(10.0);
            ui.checkbox(&mut self.use_inspector_text_mode, "Text Mode");
        });

        ScrollArea::vertical().show(ui, |ui| match self.inspect_target {
            Inspectable::Node(id) => match self.inspector_tabs {
                InspectorTab::Overview => {
                    ui.label(format!("Inspecting Node ID {}", id));

                    let current_node = &self.node_settings[id];

                    ui.heading("Position");
                    let loc = node_locations[id];
                    ui.label(format!("x: {:.3}    y: {:.3}", loc.x, loc.y));

                    ui.separator();
                    ui.heading("Node Settings");
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

                    ui.separator();

                    ui.heading("Results");

                    ui.label(format!("Reception Rate: {:.3}", self.reception_rate[id]));

                    ui.label(format!("Received: {:?}", self.received_messages[id]));

                    ui.horizontal_wrapped(|ui| {
                        ui.label("Wanted: ");

                        Frame::new()
                            .inner_margin(1.0)
                            .fill(item_background)
                            .show(ui, |ui| {
                                for message in self.wanted_messages[id].iter() {
                                    let colour = if message.was_received {
                                        Color32::GREEN
                                    } else {
                                        Color32::RED
                                    };
                                    let response =
                                        ui.colored_label(colour, message.message_id.to_string());
                                    if response.clicked() {
                                        self.transmissions
                                            .iter()
                                            .find(|x| match x.message_content {
                                                MessageContent::GeneratedMessage(id) => {
                                                    id == message.message_id
                                                }
                                                _ => false,
                                            })
                                            .map(|x| {
                                                set_time!(x.start_time);
                                            });
                                    }
                                }
                            });
                    });

                    ui.separator();
                    ui.heading("Node Events");

                    AnalysisPanel::event_ui(&self.node_events[id], ui, self.current_time.into());
                }
                InspectorTab::State => {
                    if let Some(ref mut live) = self.live_sim {
                        let this_node = live.inspect_node(id, self.current_time.into());

                        if self.use_inspector_text_mode {
                            ui.label(format!("{this_node:#?}"));
                        } else {
                            match serde_inspector::to_value(this_node) {
                                Ok(serde_value) => {
                                    serde_inspector::any_inspector(0, serde_value, ui);
                                }
                                Err(e) => {
                                    ui.label(e.to_string());
                                    ui.separator();
                                    let display = serde_json::to_string_pretty(this_node)
                                        .unwrap_or_else(|_| format!("{:#?}", this_node));

                                    ui.label(display);
                                }
                            }
                        }
                    }
                }
            },
            Inspectable::Transmission(id) => {
                ui.label(format!("Inspecting Transmission ID {}", id));

                let current_transmission = self.transmissions.iter().find(|x| x.id == id).unwrap();

                ui.label(format!(
                    "Transmitter Id: {}",
                    current_transmission.transmitter_id
                ));

                ui.label(format!(
                    "Airtime: {:.3}s -> {:.3}s  ({:.3}s)",
                    current_transmission.start_time,
                    current_transmission.end_time,
                    current_transmission.airtime()
                ))
                .clicked()
                .then(|| {
                    set_time!(current_transmission.start_time);
                });

                ui.label(format!(
                    "Bandwidth: {:.3} kHz",
                    current_transmission.bandwidth.kHz()
                ));
                ui.label(format!(
                    "Carrier Band: {:?}",
                    current_transmission.carrier_band
                ));
                ui.label(format!(
                    "Power: {:.3} dBm",
                    current_transmission.power.dbm()
                ));

                ui.label(format!("SF: {}", current_transmission.sf));

                ui.separator();
                ui.add(Label::new(RichText::new("Header").underline().weak()));
                ui.label(format!("{:#?}", current_transmission.header));

                ui.separator();
                ui.add(Label::new(RichText::new("Content").underline().weak()));
                ui.label(format!("{:#?}", current_transmission.message_content));
            }
            _ => (),
        })
    }

    fn analysis_timeline_panel(
        &mut self,
        item_background: Color32,
        main_red: Color32,
        ui: &mut egui::Ui,
    ) {
        macro_rules! set_time {
            ($new_time: expr) => {
                let new_time = $new_time.into();
                self.prev_time = self.current_time;
                self.current_time = new_time;
            };
        }

        if self.playing {
            ui.ctx().request_repaint();
            let new_time = (ui.input(|i| i.time) - self.play_offset) * self.play_timescale
                + self.play_time_offset;
            set_time!(new_time);
        }

        ui.horizontal(|ui| {
            ui.label("Timeline");
            ui.centered_and_justified(|ui| {
                ui.label(format!(
                    "Results for {} with seed {}",
                    self.used_model, self.used_seed
                ))
            })
        });

        ui.horizontal(|ui| {
            if ui.button("Prev Sim Event").clicked() {
                if let Some(e) = self
                    .sim_events
                    .iter()
                    .rev()
                    .skip_while(|x| x.time >= self.current_time.into())
                    .next()
                {
                    set_time!(e.time);
                }
            }
            if ui.button("Next Sim Event").clicked() {
                if let Some(e) = self
                    .sim_events
                    .iter()
                    .skip_while(|x| x.time <= self.current_time.into())
                    .next()
                {
                    set_time!(e.time);
                }
            }

            ui.add_space(20.0);

            if ui.button("Prev Time").clicked() {
                let new_time = (self.prev_time).into();
                self.prev_time = self.current_time;
                self.current_time = new_time;
            }

            ui.add_space(20.0);

            if ui.button("Play / Pause").clicked() {
                self.playing = !self.playing;
                self.play_offset = ui.input(|i| i.time);
                self.play_time_offset = self.current_time;
            }

            ui.label("at");
            ui.add(DragValue::new(&mut self.play_timescale).suffix("x"));
            ui.label("speed");

            ui.with_layout(Layout::default().with_cross_align(Align::RIGHT), |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(80.0);
                    ui.label(format!("Time: {:.3}s", self.current_time));
                })
            });
        });

        let mut minutes = (self.current_time / 60.0).floor();
        let mut seconds = self.current_time % 60.0;
        ui.spacing_mut().slider_width = 900.0;

        ui.style_mut().visuals.widgets.inactive = WidgetVisuals {
            bg_fill: item_background,
            weak_bg_fill: main_red,
            bg_stroke: Stroke::new(1.0, main_red),
            corner_radius: CornerRadius::ZERO,
            fg_stroke: Stroke::new(1.0, main_red),
            expansion: 0.0,
        };

        let mins_slider = egui::Slider::new(&mut minutes, 0.0..=(self.end_time / 60.0).floor())
            .handle_shape(egui::style::HandleShape::Rect { aspect_ratio: 0.4 })
            .integer()
            .text("Minutes");

        let secs_slider = egui::Slider::new(&mut seconds, 0.0..=60.0)
            .handle_shape(egui::style::HandleShape::Rect { aspect_ratio: 0.4 })
            .text("Seconds");

        ui.add(mins_slider);
        ui.add(secs_slider);

        let slider_time = minutes * 60.0 + seconds;

        if slider_time != self.current_time {
            set_time!(slider_time);
        }

        ui.add_space(5.0);
    }
}
