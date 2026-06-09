use egui::{CentralPanel, Frame, Margin, MenuBar, Panel, TextBuffer};

use egui_dock::{DockArea, DockState, NodePath, TabViewer, tab_viewer::OnCloseResponse};
use frogcore::{
    scenario::Scenario,
    simulation::{MessageContent, data_structs::LogItem},
    units::Time,
};

use macroquad::{miniquad::window::request_quit, prelude::*};
use slotmap::{SlotMap, new_key_type};

use crate::{
    debug::Debugger,
    playback_panel::PlaybackPanel,
    save_load::FileSystem,
    scenario_editor_panel::{ScenarioEditorPanel, default_scenario},
    scenario_generator_panel::ScenarioGeneratorPanel,
    style::dark_visuals,
};

mod components;
mod debug;
pub mod playback_panel;
mod save_load;
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
        files: {
            let mut files = FileSystem::new();
            files.open_scenarios_in_folder(".".into());
            files
        },
        renaming_scenario: None,
        name_buf: String::new(),
    };

    let tab_display = TabDisplay {
        store,
        tabs: SlotMap::with_key(),
    };

    let mut app = MyApp {
        tabs: DockState::new(Vec::new()),
        tab_display,
    };

    {
        let tab_id = app.tab_display.tabs.insert(Tab {
            body: TabBody::ScenarioEditor(Box::new(ScenarioEditorPanel::new(
                default_scenario(),
                None,
            ))),
        });
        app.tabs.push_to_focused_leaf(tab_id);
    }

    app.run().await;
}

#[derive(Debug)]
pub struct Tab {
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

struct MyApp {
    tabs: DockState<TabKey>,
    tab_display: TabDisplay,
}

impl MyApp {
    async fn run(mut self) {
        self.tabs.set_focused_node_and_surface(NodePath::MAIN_ROOT);
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

    fn update_egui(&mut self, base_ui: &mut egui::Ui) {
        base_ui.global_style_mut(|style| {
            style.visuals = dark_visuals();
        });

        MenuBar::new().ui(base_ui, |ui| {
            ui.add_space(10.);
            ui.menu_button("File", |ui| {
                if ui.button("Open File").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_directory("/")
                        .add_filter("Sim Files", &save_load::EXTENSIONS)
                        .pick_file()
                    {
                        let scen_key = self.tab_display.store.files.open(path).unwrap();
                        open_scenario_as_tab(
                            scen_key,
                            self.tab_display
                                .store
                                .files
                                .scenarios
                                .get_mut(scen_key)
                                .expect("Just created: must be valid"),
                            &mut self.tabs,
                            &mut self.tab_display.tabs,
                        );
                    }
                }
                ui.separator();
                if ui.button("Switch Project Folder").clicked() {
                    if let Some(path) = rfd::FileDialog::new().set_directory("/").pick_folder() {
                        self.change_project_dir(path);
                    }
                }
                ui.separator();
                if ui.button("Save").clicked() {
                    if let Some((_, tab)) = self.tabs.find_active_focused() {
                        let tab = self
                            .tab_display
                            .tabs
                            .get_mut(*tab)
                            .expect("Tab keys in DockState must be valid");

                        match &mut tab.body {
                            TabBody::ScenarioEditor(panel) => {
                                panel.save(&mut self.tab_display.store)
                            }
                            _ => (),
                        }
                    }
                }
                ui.separator();
                if ui.button("Exit").clicked() {
                    request_quit();
                }
            });
        });

        Panel::left("mode_selector")
            .default_size(150.0)
            .show_inside(base_ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.menu_button("Create New", |ui| {
                        if ui.button("Empty Custom Scenario").clicked() {
                            self.tab_display
                                .store
                                .insert_into_files("New Scenario".to_string(), default_scenario());
                        }
                        if ui.button("Custom Scenario from Generator").clicked() {
                            let generator_panel = ScenarioGeneratorPanel::new();
                            let tab_id = self.tab_display.tabs.insert(Tab {
                                body: TabBody::ScenarioGenerator(Box::new(generator_panel)),
                            });
                            self.tabs.push_to_focused_leaf(tab_id);
                        }
                    });
                });

                ui.add_space(5.);
                ui.label("Scenarios");
                self.tab_display
                    .store
                    .files
                    .scenarios
                    .iter_mut()
                    .for_each(|(key, scen)| {
                        if self
                            .tab_display
                            .store
                            .renaming_scenario
                            .is_some_and(|x| x == key)
                        {
                            let name_input =
                                ui.text_edit_singleline(&mut self.tab_display.store.name_buf);

                            if name_input.lost_focus() {
                                self.tab_display
                                    .store
                                    .global_action_queue
                                    .push(GlobalAction::CommitRename);
                            };

                            if name_input.has_focus() == false {
                                name_input.request_focus();
                            }

                            return;
                        }

                        let scen_button = ui.button(scen.name());

                        scen_button.context_menu(|ui| {
                            if ui.button("Create copy").clicked() {
                                self.tab_display.store.global_action_queue.push(
                                    GlobalAction::CreateScenario(
                                        format!("Copy of {}", scen.name()),
                                        scen.scenario().clone(),
                                    ),
                                );
                            }

                            if ui.button("Rename").clicked() {
                                self.tab_display.store.renaming_scenario = Some(key);
                                self.tab_display.store.name_buf = scen.name();
                            }

                            if ui.button("Delete").clicked() {
                                self.tab_display
                                    .store
                                    .global_action_queue
                                    .push(GlobalAction::DeleteScenario(key));
                            }
                        });

                        if scen_button.clicked() {
                            open_scenario_as_tab(
                                key,
                                scen,
                                &mut self.tabs,
                                &mut self.tab_display.tabs,
                            );
                        }
                    });
            });

        CentralPanel::default()
            .frame(Frame::NONE)
            .show_inside(base_ui, |ui| {
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
                    insert_into_files(
                        name,
                        scenario,
                        &mut self.tab_display.store.files,
                        &mut self.tab_display.store.renaming_scenario,
                        &mut self.tab_display.store.name_buf,
                    );
                }
                GlobalAction::RunScenario(linked_save, scenario) => {
                    let playback = PlaybackPanel::from_scenario(scenario, linked_save);
                    let tab_id = self.tab_display.tabs.insert(Tab {
                        body: TabBody::Analysis(Box::new(playback)),
                    });
                    self.tabs.push_to_focused_leaf(tab_id);
                }
                GlobalAction::DeleteScenario(scen_key) => {
                    self.tabs.retain_tabs(|tab_key| {
                        let tab = self.tab_display.tabs.get(*tab_key).expect("msg");
                        let output = match &tab.body {
                            TabBody::ScenarioEditor(panel) => {
                                panel.saved_data.is_none_or(|x| x != scen_key)
                            }
                            _ => true,
                        };

                        if !output {
                            self.tab_display.tabs.remove(*tab_key);
                        }

                        output
                    });
                    self.tab_display.store.files.delete(scen_key);
                }
                GlobalAction::OnCloseTab(tab_key) => {
                    self.tab_display.tabs.remove(tab_key);
                }
                GlobalAction::CommitRename => {
                    let Some(key) = self.tab_display.store.renaming_scenario.take() else {
                        warn!("FileSystem::commit_rename called while not renaming");
                        return;
                    };
                    self.tab_display
                        .store
                        .files
                        .rename(key, self.tab_display.store.name_buf.take());
                }
            });
    }

    fn change_project_dir(&mut self, path: std::path::PathBuf) {
        self.tabs.retain_tabs(|_| false);
        self.tab_display.tabs.clear();
        self.tab_display.store.files.scenarios.clear();
        self.tab_display.store.files.open_scenarios_in_folder(path);
    }
}

new_key_type! {
    pub struct TabKey;
    pub struct ScenarioKey;
}

#[derive(Debug)]
pub struct TabDisplay {
    pub store: GuiStore,
    pub tabs: SlotMap<TabKey, Tab>,
}

#[derive(Debug)]
pub struct GuiStore {
    pub node_spacing: f32,
    pub files: FileSystem,
    renaming_scenario: Option<ScenarioKey>,
    name_buf: String,
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

    pub fn insert_into_files(&mut self, name: String, scenario: Scenario) -> ScenarioKey {
        let GuiStore {
            files,
            renaming_scenario,
            name_buf,
            ..
        } = self;
        insert_into_files(name, scenario, files, renaming_scenario, name_buf)
    }
}

fn insert_into_files(
    name: String,
    scenario: Scenario,
    files: &mut FileSystem,
    renaming_scenario: &mut Option<ScenarioKey>,
    name_buf: &mut String,
) -> ScenarioKey {
    let key = files.save_new_scenario(name, scenario);
    *name_buf = files
        .get_name(key)
        .expect("Key has just been created: must be valid");
    *renaming_scenario = Some(key);
    key
}

fn open_scenario_as_tab(
    key: ScenarioKey,
    scen: &mut save_load::ScenarioFile,
    dock_state: &mut DockState<TabKey>,
    tabs: &mut SlotMap<TabKey, Tab>,
) {
    match scen
        .open_in_tab
        .and_then(|tab_key| dock_state.find_tab(&tab_key))
    {
        Some(indices) => {
            dock_state
                .set_active_tab(indices)
                .expect("Just been searched for so should be valid");
        }
        None => {
            let tab_id = tabs.insert(Tab {
                body: TabBody::ScenarioEditor(Box::new(ScenarioEditorPanel::new(
                    scen.scenario().clone(),
                    Some(key),
                ))),
            });
            dock_state.push_to_focused_leaf(tab_id);
            scen.open_in_tab = Some(tab_id);
        }
    }
}

impl TabViewer for TabDisplay {
    type Tab = self::TabKey;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        let tab = self.tabs.get(*tab).unwrap();
        match &tab.body {
            TabBody::Analysis(panel) => panel
                .linked_save
                .and_then(|key| self.store.files.get_name(key))
                .map_or("Unnamed Playback".into(), |name| {
                    format!("{} Playback", name).as_str().into()
                }),
            TabBody::ScenarioEditor(scenario_editor_panel) => scenario_editor_panel
                .saved_data
                .and_then(|key| self.store.files.get_name(key))
                .map_or("Unnamed Scenario".into(), |name| {
                    if scenario_editor_panel.dirty {
                        format!("{}*", name).into()
                    } else {
                        name.into()
                    }
                }),
            TabBody::ScenarioGenerator(_) => "Generator".into(),
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        self.tabs
            .get_mut(*tab)
            .unwrap()
            .show(*tab, ui, &mut self.store);
    }

    fn context_menu(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab, _path: NodePath) {
        let tab = self.tabs.get_mut(*tab).unwrap();

        match &mut tab.body {
            TabBody::ScenarioEditor(panel) => {
                if ui.button("Save").clicked() {
                    panel.save(&mut self.store);
                }
            }
            _ => (),
        }
    }

    fn id(&mut self, tab: &mut Self::Tab) -> egui::Id {
        egui::Id::new(tab)
    }

    fn on_tab_button(&mut self, _tab: &mut Self::Tab, _response: &egui::Response) {}

    fn closeable(&mut self, _tab: &mut Self::Tab) -> bool {
        true
    }

    fn on_close(&mut self, tab: &mut Self::Tab) -> OnCloseResponse {
        self.store.queue_action(GlobalAction::OnCloseTab(*tab));
        OnCloseResponse::Close
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

    fn is_closeable(&self, _tab: &Self::Tab) -> bool {
        true
    }

    fn on_rect_changed(&mut self, _tab: &mut Self::Tab) {}
}

#[derive(Debug, Clone)]
pub enum GlobalAction {
    CreateScenario(String, Scenario),
    RunScenario(Option<ScenarioKey>, Scenario),
    DeleteScenario(ScenarioKey),
    OnCloseTab(TabKey),
    CommitRename,
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
