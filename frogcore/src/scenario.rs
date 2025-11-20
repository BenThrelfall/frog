pub mod generation;

use serde::{Deserialize, Serialize};

use crate::{
    node_location::NodeLocation,
    scenario::generation::ScenarioGenerator,
    simulation::{data_structs::CarrierBand, models::TransmissionModel}, units::{Db, Dbm, Frequency, Power, SECONDS, Time},
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ScenarioIdentity {
    Generated {
        generator: ScenarioGenerator,
        seed: u64,
    },
    /// A custom created scenario
    Custom,
}

impl ScenarioIdentity {
    pub fn create(&self) -> Scenario {
        match self {
            ScenarioIdentity::Custom => panic!("Cannot regenerate custom scenario"),
            ScenarioIdentity::Generated { generator, seed } => {
                let mut output = generator.generate_from_seed(*seed);
                output.identity = self.clone();
                output
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    // Regeneration
    pub identity: ScenarioIdentity,

    // Data values
    pub map: NodeLocation,
    pub model: TransmissionModel,
    pub messages: Vec<ScenarioMessage>,
    pub settings: Vec<ScenarioNodeSettings>,
}

impl Scenario {
    /// Legacy use `scenario.settings` directly instead
    pub fn get_settings(&self) -> Vec<ScenarioNodeSettings> {
        self.settings.clone()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioMessage {
    /// who the message will be sent by
    pub sender: usize,

    /// who needs to receive the message
    pub targets: Vec<usize>,

    /// at what sim time will the message be sent in seconds
    pub generate_time: Time,

    /// size in bytes of the pretend message content (not including header)
    pub size: i32,

    /// markers indicating if this is a special type of message
    pub markers: Vec<MessageMarker>,

    /// enque generation for the same message this many times
    pub num_generations: u32,
    /// message is generated at `send_time + generation_spacing * gen` where `gen = 0..num_generations`
    pub generation_spacing: Time,
}

impl ScenarioMessage {
    pub fn new(sender: usize, targets: Vec<usize>, generate_time: Time, size: i32) -> Self {
        Self {
            sender,
            targets,
            generate_time,
            size,
            markers: Vec::new(),
            num_generations: 1,
            generation_spacing: 1.0 * SECONDS,
        }
    }

    pub fn with_marker(mut self, marker: MessageMarker) -> Self {
        self.markers.push(marker);
        self
    }

    pub fn with_repeats(mut self, total_generations: u32, spacing: Time) -> Self {
        self.num_generations = total_generations;
        self.generation_spacing = spacing;
        self
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MessageMarker {
    Emergency,
}



#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MovementIndicator {
    Unset,
    Mobile,
    Stationary,
}

impl MovementIndicator {
    pub const VALUES: [MovementIndicator; 3] = [
        MovementIndicator::Unset,
        MovementIndicator::Mobile,
        MovementIndicator::Stationary,
    ];
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioNodeSettings {
    /// Number of bits per transmission symbol. Known as spreading factor.
    pub sf: i32,

    /// bandwidth in kHz
    pub bandwidth: Frequency,

    /// Number of bits used for error checking per 4 real bits.
    /// Must, of course, be at least 4 and usually at most 8.
    pub coding_rate: i32,

    pub is_gateway: bool,
    pub movement_indicator: MovementIndicator,

    /// Isotropic radiated power in dBm.
    /// Everything is modelled with an isotropic antenna
    pub max_power: Db<Power>,
    pub carrier_band: CarrierBand,

    /// Time in milleseconds
    pub reaction_time: Time,
}

impl Default for ScenarioNodeSettings {
    /// Default using LongFast settings
    /// <https://meshtastic.org/docs/overview/radio-settings/>
    ///
    /// ```
    /// # use frogcore::scenario::*;
    /// # use frogcore::units::*;
    /// # use frogcore::simulation::data_structs::*;
    /// ScenarioNodeSettings{
    ///     sf: 11,
    ///     max_power: Dbm::from_dbm(22.0),
    ///     carrier_band: CarrierBand::B868,
    ///     bandwidth: Frequency::from_kHz(250.0),
    ///     reaction_time: Time::from_milis(0.1),
    ///     coding_rate: 5,
    ///     is_gateway: false,
    ///     movement_indicator: MovementIndicator::Unset,
    /// };
    /// ```
    ///
    fn default() -> Self {
        Self {
            sf: 11,
            max_power: Dbm::from_dbm(22.0),
            carrier_band: CarrierBand::B868,
            bandwidth: Frequency::from_kHz(250.0),
            reaction_time: Time::from_milis(0.1),
            coding_rate: 5,
            is_gateway: false,
            movement_indicator: MovementIndicator::Unset,
        }
    }
}

impl ScenarioNodeSettings {
    pub fn with_movement_indicator(mut self, indicator: MovementIndicator) -> ScenarioNodeSettings {
        self.movement_indicator = indicator;
        self
    }

    pub fn as_gateway(mut self) -> ScenarioNodeSettings {
        self.is_gateway = true;
        self
    }
}
