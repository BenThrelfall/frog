use std::cell::RefCell;
use std::sync::Arc;

use egui::{CentralPanel, ComboBox, Frame, Modal, RichText, TopBottomPanel, vec2};

use frogcore::{
    node::{MODEL_LIST, ModelSelection},
    scenario::Scenario,
    sim_file::write_file,
    simulation::{MessageContent, data_structs::LogItem},
    units::Time,
};

use macroquad::prelude::*;

use crate::{
    analysis_panel::AnalysisPanel, browser_panel::BrowserPanel,
    scenario_editor_panel::ScenarioEditorPanel, scenario_generator_panel::ScenarioGeneratorPanel,
    style::dark_visuals,
};

pub mod analysis_panel;
pub mod browser_panel;
mod components;
pub mod scenario_editor_panel;
mod scenario_generator_panel;
mod scene;
pub mod style;

fn window_conf() -> macroquad::conf::Conf {
    macroquad::conf::Conf {
        miniquad_conf: Conf {
            window_title: "frogcore".to_owned(),
            window_width: 1600,
            window_height: 960,
            ..Default::default()
        },
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let store = Arc::new(RefCell::new(GuiStore {
        node_spacing: 1.0,
        global_action: GlobalAction::None,
    }));

    let (main_panel, editor_panel) = (None, Some(scenario_editor_panel::new_scenario_and_panel()));

    let active_tab = if main_panel.is_some() {
        Tabs::Analysis
    } else {
        Tabs::ScenarioEditor
    };

    let browser_panel = BrowserPanel::new(store.clone());
    let generator_panel = ScenarioGeneratorPanel::new(store.clone());

    let app = MyApp {
        main_panel,
        active_tab,
        editor_panel,
        save_path: "output.json".to_owned(),
        model_selection: ModelSelection::Meshtastic,
        new_modal_open: false,
        store,
        browser_panel,
        generator_panel,
    };

    app.run().await;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tabs {
    Analysis,
    ScenarioEditor,
    ScenarioGenerator,
    Browser,
}

struct MyApp {
    main_panel: Option<AnalysisPanel>,
    editor_panel: Option<ScenarioEditorPanel>,
    generator_panel: ScenarioGeneratorPanel,
    browser_panel: BrowserPanel,
    model_selection: ModelSelection,
    new_modal_open: bool,
    active_tab: Tabs,
    save_path: String,
    store: Arc<RefCell<GuiStore>>,
}

impl MyApp {
    async fn run(mut self) {
        loop {
            clear_background(Color::from_hex(0x404040));
            self.update();

            set_default_camera();
            egui_macroquad::draw();

            next_frame().await;
        }
    }

    fn update(&mut self) {
        egui_macroquad::ui(|ctx| self.update_egui(ctx));
    }

    fn update_egui(&mut self, ctx: &egui::Context) {
        ctx.style_mut(|style| {
            style.visuals = dark_visuals();
        });

        TopBottomPanel::top("main_top")
            .default_height(36.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.allocate_ui_with_layout(
                        vec2(200., 36.),
                        egui::Layout::top_down(egui::Align::Center),
                        |ui| {
                            ui.label(RichText::new("Tabs").color(egui::Color32::GRAY));
                            ui.horizontal(|ui| {
                                if ui
                                    .selectable_label(
                                        self.active_tab == Tabs::ScenarioEditor,
                                        "Editor",
                                    )
                                    .clicked()
                                {
                                    self.active_tab = Tabs::ScenarioEditor;
                                }
                                if ui
                                    .selectable_label(
                                        self.active_tab == Tabs::ScenarioGenerator,
                                        "Generator",
                                    )
                                    .clicked()
                                {
                                    let timestamp = get_time().to_ne_bytes();
                                    let seed = u64::from_ne_bytes(timestamp);
                                    macroquad::rand::srand(seed);

                                    self.active_tab = Tabs::ScenarioGenerator;
                                }
                                if ui
                                    .selectable_label(self.active_tab == Tabs::Analysis, "Analysis")
                                    .clicked()
                                {
                                    self.active_tab = Tabs::Analysis;
                                }
                                if ui
                                    .selectable_label(self.active_tab == Tabs::Browser, "Browser")
                                    .clicked()
                                {
                                    self.browser_panel.refresh();
                                    self.active_tab = Tabs::Browser;
                                }
                            })
                        },
                    );
                    ui.separator();

                    ui.add_space(5.0);
                    if ui.button("New Scenario").clicked() {
                        self.new_modal_open = true;
                    }
                    ui.add_space(5.0);

                    if self.new_modal_open {
                        let modal = Modal::new("New Modal".into()).show(ui.ctx(), |ui| {
                            ui.heading("Create new scenario? Current scenario will be discarded");

                            ui.horizontal_centered(|ui| {
                                if ui.button("Confirm").clicked() {
                                    self.editor_panel =
                                        Some(scenario_editor_panel::new_scenario_and_panel());
                                    self.active_tab = Tabs::ScenarioEditor;
                                    self.new_modal_open = false;
                                };
                                if ui.button("Cancel").clicked() {
                                    self.new_modal_open = false;
                                }
                            });
                        });

                        if modal.should_close() {
                            self.new_modal_open = false;
                        }
                    }

                    if let Some(ref panel) = self.editor_panel {
                        ui.vertical(|ui| {
                            if ui.button("Save Scenario As:").clicked() {
                                write_file(
                                    self.save_path.clone().into(),
                                    panel.scenario.clone(),
                                    false,
                                )
                                .unwrap();
                            }
                            ui.text_edit_singleline(&mut self.save_path);
                        });

                        ui.separator();

                        if ui.button("Run Scenario").clicked() {
                            self.main_panel = Some(AnalysisPanel::from_scenario(
                                panel.scenario.clone(),
                                self.model_selection.into(),
                            ));
                            self.active_tab = Tabs::Analysis;
                        }

                        ui.label("with");

                        ComboBox::from_label("Model")
                            .selected_text(format!("{:?}", self.model_selection))
                            .show_ui(ui, |ui| {
                                for model in MODEL_LIST {
                                    ui.selectable_value(
                                        &mut self.model_selection,
                                        model,
                                        format!("{:?}", model),
                                    );
                                }
                            });
                    }
                });
            });

        CentralPanel::default()
            .frame(Frame::NONE)
            .show(ctx, |ui| match self.active_tab {
                Tabs::Analysis => {
                    if let Some(ref mut panel) = self.main_panel {
                        ui.add(panel);
                    } else {
                        ui.centered_and_justified(|ui| {
                            ui.heading(
                                "No active analysis.\nRun the current scenario from the top panel.",
                            );
                        });
                    }
                }
                Tabs::ScenarioEditor => {
                    if let Some(ref mut panel) = self.editor_panel {
                        ui.add(panel);
                    } else {
                        ui.centered_and_justified(|ui| {
                            ui.heading("No Active Scenario");
                        });
                    }
                }
                Tabs::Browser => {
                    ui.add(&mut self.browser_panel);
                }
                Tabs::ScenarioGenerator => {
                    ui.add(&mut self.generator_panel);
                }
            });

        match &self.store.borrow().global_action {
            GlobalAction::None => (),
            GlobalAction::SetScenario(scenario) => {
                self.editor_panel = Some(ScenarioEditorPanel::new(scenario.clone()));
                self.active_tab = Tabs::ScenarioEditor;
            }
            GlobalAction::RunScenario(scenario) => {
                self.editor_panel = Some(ScenarioEditorPanel::new(scenario.clone()));
                self.main_panel = Some(AnalysisPanel::from_scenario(
                    self.editor_panel.as_ref().unwrap().scenario.clone(),
                    self.model_selection.into(),
                ));
                self.active_tab = Tabs::Analysis;
            }
        }

        self.store.borrow_mut().global_action = GlobalAction::None;
    }
}

#[derive(Debug, Clone)]
pub struct GuiStore {
    pub node_spacing: f32,

    pub global_action: GlobalAction,
}

#[derive(Debug, Clone)]
pub enum GlobalAction {
    None,
    SetScenario(Scenario),
    RunScenario(Scenario),
}

const BACK_TIME: Time = Time::from_seconds(1.0);
const FORWARD_TIME: Time = Time::from_seconds(1.0);

trait HasTime {
    fn time(&self) -> Time;
}

impl HasTime for LogItem {
    fn time(&self) -> Time {
        self.time
    }
}

fn get_event_window<T>(events: &Vec<T>, time: Time) -> impl Iterator<Item = &T>
where
    T: HasTime,
{
    events
        .iter()
        .skip_while(move |x| x.time() < time - BACK_TIME)
        .take_while(move |x| x.time() < time + FORWARD_TIME)
}
fn short_content(content: &MessageContent) -> String {
    match content {
        MessageContent::GeneratedMessage(id) => format!("Message({id})"),
        MessageContent::NodeMessage(_) => "Other".to_string(),
        MessageContent::Empty => "Empty".to_string(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Inspectable {
    Nothing,
    Node(usize),
    Transmission(u32),
}

fn convert_rect(rect_in: egui::Rect) -> Rect {
    let egui::Rect { min, max } = rect_in;
    Rect::new(min.x, min.y, max.x - min.x, max.y - min.y)
}
