use std::{fs::read_dir, path::PathBuf};

use frogcore::{
    scenario::Scenario,
    sim_file::{load_file, write_file},
};
use slotmap::SlotMap;

use crate::{ScenarioKey, TabKey};

#[derive(Debug)]
pub struct FileSystem {
    pub scenarios: SlotMap<ScenarioKey, ScenarioFile>,
}

pub const EXTENSIONS: [&str; 5] = ["json", "frog", "sim", "simpack", "rmp"];

fn read_sim_files(root: PathBuf) -> Vec<PathBuf> {
    let Ok(dir) = read_dir(root) else {
        return Vec::new();
    };

    dir.filter_map(|x| x.ok().map(|inner| inner.path()))
        .filter(|x| x.extension().map(|ext| in_extensions(ext)).unwrap_or(false))
        .collect()
}

fn in_extensions(s: &std::ffi::OsStr) -> bool {
    EXTENSIONS.iter().any(|x| s.eq_ignore_ascii_case(x))
}

impl FileSystem {
    pub fn new() -> Self {
        FileSystem {
            scenarios: SlotMap::with_key(),
        }
    }

    /// Does not traverse sub-folders
    pub fn open_scenarios_in_folder(&mut self, root: PathBuf) {
        let candidates = read_sim_files(root);

        let fs_scenarios: Vec<_> = candidates
            .into_iter()
            .filter_map(|path| {
                load_file::<Scenario>(path.clone())
                    .ok()
                    .map(|one| (one, path))
            })
            .collect();

        for (scenario, path) in fs_scenarios {
            self.scenarios.insert(ScenarioFile {
                open_in_tab: None,
                scenario,
                path,
            });
        }
    }

    pub fn open(&mut self, path: PathBuf) -> Result<ScenarioKey, String> {
        let Ok(c_path) = path.canonicalize() else {
            return Err("Cannot open file".to_string());
        };

        if let Some((key, _)) = self
            .scenarios
            .iter()
            .find(|(_, x)| x.path.canonicalize().is_ok_and(|ni| ni == c_path))
        {
            return Ok(key);
        };

        let Ok(scenario) = load_file::<Scenario>(path.clone()) else {
            return Err("Cannot open file".to_string());
        };

        let output = self.scenarios.insert(ScenarioFile {
            open_in_tab: None,
            scenario,
            path,
        });

        Ok(output)
    }

    pub fn delete(&mut self, key: ScenarioKey) {
        let file = self.scenarios.remove(key).unwrap();
        std::fs::remove_file(file.path).unwrap();
    }

    pub fn rename(&mut self, key: ScenarioKey, name: String) {
        let file = self.scenarios.get_mut(key).unwrap();
        let mut swap_path = file.path.with_file_name(format!("{name}.frog"));
        std::mem::swap(&mut file.path, &mut swap_path);
        std::fs::rename(swap_path, &file.path).unwrap();
    }

    pub fn get_name(&self, key: ScenarioKey) -> Option<String> {
        self.scenarios.get(key).map(|x| x.name())
    }

    pub fn get_scenario(&self, key: ScenarioKey) -> Option<&Scenario> {
        self.scenarios.get(key).map(|x| &x.scenario)
    }

    pub fn save_over(&mut self, key: ScenarioKey, scenario: Scenario) {
        let save = self.scenarios.get_mut(key).unwrap();
        save.scenario = scenario;
        save.save_to_fs();
    }

    pub fn save_new_scenario(&mut self, name: String, scenario: Scenario) -> ScenarioKey {
        let mut path: PathBuf = format!("{name}.frog").into();
        let mut counter = 0;

        while std::fs::exists(&path).unwrap() {
            counter += 1;
            path = format!("{name} {counter}.frog").into();
        }

        let save = ScenarioFile {
            open_in_tab: None,
            scenario,
            path,
        };
        save.save_to_fs();
        let key = self.scenarios.insert(save);

        key
    }
}

#[derive(Debug)]
pub struct ScenarioFile {
    pub open_in_tab: Option<TabKey>,
    scenario: Scenario,
    path: PathBuf,
}

impl ScenarioFile {
    pub fn name(&self) -> String {
        self.path
            .file_prefix()
            .expect("Must have a file name")
            .to_string_lossy()
            .to_string()
    }

    pub fn scenario(&self) -> &Scenario {
        &self.scenario
    }

    pub fn save_to_fs(&self) {
        write_file(self.path.clone(), &self.scenario, false).unwrap();
    }
}
