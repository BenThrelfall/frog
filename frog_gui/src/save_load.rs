use frogcore::scenario::Scenario;
use slotmap::SlotMap;

use crate::{ScenarioKey, TabKey};

#[derive(Debug)]
pub struct FileSystem {
    pub scenarios: SlotMap<ScenarioKey, ScenarioFile>,
    
}

impl FileSystem {
    pub fn new() -> Self {
        FileSystem {
            scenarios: SlotMap::with_key(),
        }
    }

    pub fn rename(&mut self, key: ScenarioKey, name: String) {
        self.scenarios.get_mut(key).unwrap().name = name;
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
    }

    pub fn save_new_scenario(&mut self, name: String, scenario: Scenario) -> ScenarioKey {
        let mut use_name = name.clone();
        let mut counter = 0;

        while self
            .scenarios
            .iter()
            .find(|(_, x)| x.name == use_name)
            .is_some()
        {
            counter += 1;
            use_name = format!("{name} {counter}");
        }

        let key = self.scenarios.insert(ScenarioFile {
            open_in_tab: None,
            scenario,
            name: use_name,
        });

        key
    }
}

#[derive(Debug)]
pub struct ScenarioFile {
    pub open_in_tab: Option<TabKey>,
    scenario: Scenario,
    name: String,
}

impl ScenarioFile {
    pub fn name(&self) -> String {
        self.name.clone()
    }

    pub fn scenario(&self) -> &Scenario {
        &self.scenario
    }
}
