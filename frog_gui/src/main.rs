use egui::{CentralPanel, CollapsingHeader, Frame, Margin, SidePanel};

use egui_dock::{DockArea, DockState, TabViewer};
use frogcore::{
    node::NodeModel,
    scenario::Scenario,
    simulation::{MessageContent, data_structs::LogItem},
    units::Time,
};

use macroquad::prelude::*;
use slotmap::{SlotMap, new_key_type};

use crate::{
    debug::Debugger,
    playback_panel::PlaybackPanel,
    scenario_editor_panel::{ScenarioEditorPanel, default_scenario},
    scenario_generator_panel::ScenarioGeneratorPanel,
    style::dark_visuals,
};

mod components;
mod debug;
pub mod playback_panel;
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
    let store = GuiStore {
        node_spacing: 1.0,
        global_action_queue: Vec::new(),
        next_id: 0,
        debugger: Debugger::new(),
    };

    let tab_display = TabDisplay {
        store,
        tabs: SlotMap::with_key(),
    };

    let app = MyApp {
        tabs: DockState::new(Vec::new()),
        scenarios: Vec::new(),
        tab_display,
        renaming_scenario: None,
    };

    app.run().await;
}

#[derive(Debug)]
pub struct Tab {
    name: String,
    body: TabBody,
}

#[derive(Debug)]
enum TabBody {
    Analysis(Box<PlaybackPanel>),
    ScenarioEditor(Box<ScenarioEditorPanel>),
    ScenarioGenerator(Box<ScenarioGeneratorPanel>),
}

impl Tab {
    fn show(&mut self, id: TabKey, ui: &mut egui::Ui, store: &mut GuiStore) -> egui::Response {
        ui.push_id(id, |ui| match &mut self.body {
            TabBody::Analysis(analysis_panel) => analysis_panel.show(ui, store),
            TabBody::ScenarioEditor(scenario_editor_panel) => scenario_editor_panel.show(ui, store),
            TabBody::ScenarioGenerator(scenario_generator_panel) => {
                scenario_generator_panel.show(ui, store)
            }
        })
        .response
    }
}

struct LoadedScenario {
    open_in_tab: Option<TabKey>,
    scenario: Scenario,
    name: String,
}

struct MyApp {
    tabs: DockState<TabKey>,
    scenarios: Vec<LoadedScenario>,
    renaming_scenario: Option<usize>,
    tab_display: TabDisplay,
}

impl MyApp {
    async fn run(mut self) {
        loop {
            self.tab_display.store.debugger.reset();
            clear_background(Color::from_hex(0x404040));
            self.update();

            set_default_camera();
            egui_macroquad::draw();
            self.tab_display.store.debugger.render_debug();

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
                                    ui.close_menu();
                                }

                                if ui.button("Rename").clicked() {
                                    self.renaming_scenario = Some(n);
                                    ui.close_menu();
                                }
                            });

                            if scen_button.clicked() {
                                match scen.open_in_tab {
                                    Some(tab_id) => {
                                        match self.tabs.find_tab(&tab_id) {
                                            Some(indices) => self.tabs.set_active_tab(indices),
                                            None => self.tabs.push_to_focused_leaf(tab_id),
                                        };
                                    }
                                    None => {
                                        let tab_id = self.tab_display.tabs.insert(Tab {
                                            name: scen.name.clone(),
                                            body: TabBody::ScenarioEditor(Box::new(
                                                ScenarioEditorPanel::new(scen.scenario.clone()),
                                            )),
                                        });
                                        self.tabs.push_to_focused_leaf(tab_id);
                                        scen.open_in_tab = Some(tab_id);
                                    }
                                }
                            }
                        });
                    });
            });

        CentralPanel::default().frame(Frame::NONE).show(ctx, |ui| {
            let mut style = egui_dock::Style::from_egui(ui.style());
            style.tab.tab_body.inner_margin = Margin::ZERO;
            DockArea::new(&mut self.tabs)
                .style(style)
                .show_leaf_collapse_buttons(false)
                .show_leaf_close_all_buttons(false)
                .show_inside(ui, &mut self.tab_display);
        });

        self.tab_display
            .store
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
                GlobalAction::RunScenario(scenario, model) => {
                    let playback = PlaybackPanel::from_scenario(scenario, model);
                    let tab_id = self.tab_display.tabs.insert(Tab {
                        name: "New Playback".to_string(),
                        body: TabBody::Analysis(Box::new(playback)),
                    });
                    self.tabs.push_to_focused_leaf(tab_id);
                }
            });
    }
}

new_key_type! {
    pub struct TabKey;
}

#[derive(Debug)]
pub struct TabDisplay {
    pub store: GuiStore,
    pub tabs: SlotMap<TabKey, Tab>,
}

#[derive(Debug)]
pub struct GuiStore {
    pub node_spacing: f32,
    pub next_id: u64,
    pub global_action_queue: Vec<GlobalAction>,
    pub debugger: Debugger,
}

impl GuiStore {
    pub fn new_id(&mut self) -> u64 {
        let output = self.next_id;
        self.next_id += 1;
        output
    }

    pub fn queue_action(&mut self, action: GlobalAction) {
        self.global_action_queue.push(action);
    }
}

impl TabViewer for TabDisplay {
    type Tab = self::TabKey;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        self.tabs.get(*tab).unwrap().name.as_str().into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        self.tabs
            .get_mut(*tab)
            .unwrap()
            .show(*tab, ui, &mut self.store);
    }

    fn context_menu(
        &mut self,
        _ui: &mut egui::Ui,
        _tab: &mut Self::Tab,
        _surface: egui_dock::SurfaceIndex,
        _node: egui_dock::NodeIndex,
    ) {
    }

    fn id(&mut self, tab: &mut Self::Tab) -> egui::Id {
        egui::Id::new(tab)
    }

    fn on_tab_button(&mut self, _tab: &mut Self::Tab, _response: &egui::Response) {}

    fn closeable(&mut self, _tab: &mut Self::Tab) -> bool {
        true
    }

    fn on_close(&mut self, _tab: &mut Self::Tab) -> bool {
        true
    }

    fn on_add(&mut self, _surface: egui_dock::SurfaceIndex, _node: egui_dock::NodeIndex) {}

    fn add_popup(
        &mut self,
        _ui: &mut egui::Ui,
        _surface: egui_dock::SurfaceIndex,
        _node: egui_dock::NodeIndex,
    ) {
    }

    fn force_close(&mut self, _tab: &mut Self::Tab) -> bool {
        false
    }

    fn tab_style_override(
        &self,
        _tab: &Self::Tab,
        _global_style: &egui_dock::TabStyle,
    ) -> Option<egui_dock::TabStyle> {
        None
    }

    fn allowed_in_windows(&self, _tab: &mut Self::Tab) -> bool {
        false
    }

    fn clear_background(&self, _tab: &Self::Tab) -> bool {
        false
    }

    fn scroll_bars(&self, _tab: &Self::Tab) -> [bool; 2] {
        [false, false]
    }
}

#[derive(Debug, Clone)]
pub enum GlobalAction {
    CreateScenario(String, Scenario),
    RunScenario(Scenario, NodeModel),
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
