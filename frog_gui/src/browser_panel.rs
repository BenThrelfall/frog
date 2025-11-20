use std::{cell::RefCell, fs::read_dir, path::PathBuf, sync::Arc};

use egui::{CentralPanel, ScrollArea, SidePanel, Widget};

use frogcore::{
    scenario::{Scenario, ScenarioIdentity},
    sim_file::{load_file, SimOutput},
};

use crate::{GlobalAction, GuiStore};
use serde_inspector::AnyInspector;

pub struct BrowserPanel {
    store: Arc<RefCell<GuiStore>>,
    sim_files: Vec<PathBuf>,
    active_file: Option<usize>,
    inspect_file: InspectableFile,
}

impl BrowserPanel {
    pub fn new(store: Arc<RefCell<GuiStore>>) -> BrowserPanel {
        let sim_files = read_sim_files();

        BrowserPanel {
            store,
            sim_files,
            active_file: None,
            inspect_file: InspectableFile::Nothing,
        }
    }

    pub fn refresh(&mut self) {
        let sim_files = read_sim_files();
        self.sim_files = sim_files;
        self.active_file = None;
        self.inspect_file = InspectableFile::Nothing;
    }
}

const EXTENSIONS: [&str; 4] = ["json", "sim", "simpack", "rmp"];

fn read_sim_files() -> Vec<PathBuf> {
    let Ok(dir) = read_dir(".") else {
        return Vec::new();
    };

    dir.filter_map(|x| x.ok().map(|inner| inner.path()))
        .filter(|x| x.extension().map(|ext| in_extensions(ext)).unwrap_or(false))
        .collect()
}

fn in_extensions(s: &std::ffi::OsStr) -> bool {
    EXTENSIONS.iter().any(|x| s.eq_ignore_ascii_case(x))
}

impl Widget for &mut BrowserPanel {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        SidePanel::left("file_browser").show_inside(ui, |ui| {
            if ui.button("Refresh").clicked() {
                self.refresh();
            }

            self.sim_files.iter().enumerate().for_each(|(index, path)| {
                if ui
                    .selectable_label(
                        self.active_file.map(|x| x == index).unwrap_or(false),
                        path.file_name().unwrap().to_str().unwrap(),
                    )
                    .clicked()
                {
                    self.active_file = Some(index);

                    let inspectable = if let Ok(inner) = load_file(path.clone()) {
                        InspectableFile::ScenarioIdentity(inner)
                    } else if let Ok(inner) = load_file(path.clone()) {
                        InspectableFile::Simpack(inner)
                    } else if let Ok(inner) = load_file(path.clone()) {
                        InspectableFile::Scenario(inner)
                    } else if let Ok(inner) = load_file(path.clone()) {
                        InspectableFile::Results(inner)
                    } else {
                        InspectableFile::Nothing
                    };

                    self.inspect_file = inspectable;
                }
            });
        });

        CentralPanel::default().show_inside(ui, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                let Some(active_file) = self.active_file else {
                    return;
                };

                ui.heading(
                    self.sim_files[active_file]
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap(),
                );

                match &self.inspect_file {
                    InspectableFile::Nothing => (),
                    InspectableFile::Simpack(identities) => {
                        for (index, scen_id) in identities.iter().enumerate() {
                            ui.separator();

                            if ui.button("Load").clicked() {
                                self.store.borrow_mut().global_action =
                                    GlobalAction::SetScenario(scen_id.create())
                            }

                            let val = serde_inspector::to_value(scen_id).unwrap();
                            ui.add(&mut AnyInspector::new(val, index as u64));
                            ui.separator();
                        }
                    }
                    InspectableFile::ScenarioIdentity(identity) => {
                        if ui.button("Load").clicked() {
                            self.store.borrow_mut().global_action =
                                GlobalAction::SetScenario(identity.create())
                        }

                        let val = serde_inspector::to_value(identity).unwrap();
                        ui.add(&mut AnyInspector::new(val, 0));
                    }
                    InspectableFile::Scenario(scenario) => {
                        let identity = &scenario.identity;

                        if ui.button("Load").clicked() {
                            self.store.borrow_mut().global_action =
                                GlobalAction::SetScenario(scenario.clone())
                        }

                        let val = serde_inspector::to_value(identity).unwrap();
                        ui.add(&mut AnyInspector::new(val, 0));
                    },
                    InspectableFile::Results(sim_output) => {
                        let scenario_id = &sim_output.complete_identity.scenario_identity;

                        if ui.button("Load").clicked() {
                            self.store.borrow_mut().global_action =
                                GlobalAction::SetScenario(scenario_id.create())
                        }

                        let val = serde_inspector::to_value(scenario_id).unwrap();
                        ui.add(&mut AnyInspector::new(val, 0));
                    },
                }
            })
        });

        ui.response()
    }
}

#[derive(Debug, Clone)]
enum InspectableFile {
    Nothing,
    Simpack(Vec<ScenarioIdentity>),
    ScenarioIdentity(ScenarioIdentity),
    Scenario(Scenario),
    Results(SimOutput),
}
