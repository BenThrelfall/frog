use std::sync::Arc;
use std::{cell::RefCell, fmt::format};

use egui::{
    CentralPanel, CollapsingHeader, ComboBox, Frame, Modal, RichText, SidePanel, TopBottomPanel,
    Widget, vec2,
};

use frogcore::{
    node::{MODEL_LIST, ModelSelection},
    scenario::Scenario,
    sim_file::write_file,
    simulation::{MessageContent, data_structs::LogItem},
    units::Time,
};

use macroquad::prelude::*;

use crate::{
    analysis_panel::AnalysisPanel,
    browser_panel::BrowserPanel,
    scenario_editor_panel::{ScenarioEditorPanel, default_scenario, new_scenario_and_panel},
    scenario_generator_panel::ScenarioGeneratorPanel,
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

    let app = MyApp {
        tabs: Vec::new(),
        scenarios: Vec::new(),
        active_tab: 0,
        save_path: "output.json".to_owned(),
        model_selection: ModelSelection::Meshtastic,
        new_modal_open: false,
        store,
        renaming_scenario: None,
    };

    app.run().await;
}

struct Tab {
    name: String,
    body: TabBody,
}

enum TabBody {
    Analysis(Box<AnalysisPanel>),
    ScenarioEditor(Box<ScenarioEditorPanel>),
    ScenarioGenerator(Box<ScenarioGeneratorPanel>),
    Browser(Box<BrowserPanel>),
}

impl Tab {
    fn show(&mut self, ui: &mut egui::Ui) -> egui::Response {
        match &mut self.body {
            TabBody::Analysis(analysis_panel) => ui.add(analysis_panel.as_mut()),
            TabBody::ScenarioEditor(scenario_editor_panel) => {
                ui.add(scenario_editor_panel.as_mut())
            }
            TabBody::ScenarioGenerator(scenario_generator_panel) => {
                ui.add(scenario_generator_panel.as_mut())
            }
            TabBody::Browser(browser_panel) => ui.add(browser_panel.as_mut()),
        }
    }
}

struct LoadedScenario {
    open_in_tab: Option<usize>,
    scenario: Scenario,
    name: String,
}

struct MyApp {
    tabs: Vec<Tab>,
    scenarios: Vec<LoadedScenario>,
    model_selection: ModelSelection,
    new_modal_open: bool,
    active_tab: usize,
    save_path: String,
    renaming_scenario: Option<usize>,
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

        SidePanel::left("mode_selector")
            .default_width(80.0)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.menu_button("Create New", |ui| {
                        if ui.button("Empty Custom Scenario").clicked() {
                            self.scenarios.push(LoadedScenario {
                                open_in_tab: None,
                                scenario: default_scenario(),
                                name: "New Scenario".to_string(),
                            });
                            self.renaming_scenario = Some(self.scenarios.len() - 1);
                            ui.close_menu();
                        }
                        if ui.button("Custom Scenario from Generator").clicked() {
                            ui.close_menu();
                        }
                        if ui.button("Study").clicked() {
                            ui.close_menu();
                        }
                    });
                });

                CollapsingHeader::new("Custom Scenarios")
                    .default_open(true)
                    .show(ui, |ui| {
                        self.scenarios.iter_mut().enumerate().for_each(|(n, scen)| {
                            if self.renaming_scenario.is_some_and(|x| x == n) {
                                let name_input = ui.text_edit_singleline(&mut scen.name);

                                if name_input.lost_focus() {
                                    self.renaming_scenario = None;
                                };

                                if name_input.has_focus() == false {
                                    name_input.request_focus();
                                }

                                return;
                            }

                            let scen_button = ui.button(&scen.name);

                            scen_button.context_menu(|ui| {
                                if ui.button("Create copy").clicked() {
                                    self
                                    ui.close_menu();
                                }

                                if ui.button("Rename").clicked() {
                                    self.renaming_scenario = Some(n);
                                    ui.close_menu();
                                }
                            });

                            if scen_button.clicked() {
                                match scen.open_in_tab {
                                    Some(tab_id) => self.active_tab = tab_id,
                                    None => {
                                        self.tabs.push(Tab {
                                            name: scen.name.clone(),
                                            body: TabBody::ScenarioEditor(Box::new(
                                                ScenarioEditorPanel::new(scen.scenario.clone()),
                                            )),
                                        });
                                        let tab_id = self.tabs.len() - 1;
                                        scen.open_in_tab = Some(tab_id);
                                        self.active_tab = tab_id;
                                    }
                                }
                            }
                        });
                    });
            });

        CentralPanel::default().frame(Frame::NONE).show(ctx, |ui| {
            TopBottomPanel::top("tab_bar").show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    self.tabs.iter().enumerate().for_each(|(n, tab)| {
                        ui.selectable_value(&mut self.active_tab, n, &tab.name);
                    });
                });
            });

            match self.tabs.get_mut(self.active_tab) {
                Some(tab) => {
                    tab.show(ui);
                }
                None => (),
            }
        });

        self.store
            .borrow_mut()
            .global_action_queue
            .drain(..)
            .for_each(|action| match action {
                GlobalAction::CreateScenario(name, scenario) => {
                    self.scenarios.push(LoadedScenario {
                        open_in_tab: None,
                        scenario,
                        name,
                    })
                }
            });
    }
}

#[derive(Debug, Clone)]
pub struct GuiStore {
    pub node_spacing: f32,

    pub global_action_queue: Vec<GlobalAction>,
}

#[derive(Debug, Clone)]
pub enum GlobalAction {
    CreateScenario(String, Scenario),
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
