use std::{fmt::Display, ops::Neg};

use serde::{Deserialize, Serialize};

use crate::{
    node::{Destination, Header, NodeThread, Notification},
    scenario::{ScenarioNodeSettings, MovementIndicator},
    simulation::MessageContent,
    units::*,
};

#[derive(Debug, Clone)]
pub struct NodeSettings {
    pub sf: i32,
    pub bandwidth: Frequency,

    /// Number of bits used for error checking per 4 real bits.
    /// Must, of course, be at least 4 and usually at most 8.
    pub coding_rate: i32,
    pub(super) clock_offset: Time,

    /// Indicates the node is a gateway so may generate and
    /// receieve more messages than other nodes.
    pub is_gateway: bool,

    /// Indicates if the node will move or remain stationary.
    /// In real life this is set by the owner of the node.
    /// Or potentially updates automatically if gps is available.
    pub movement_indicator: MovementIndicator,

    /// Isotropic radiated power.
    /// Everything is modelled with an isotropic antenna
    pub max_power: Db<Power>,
    pub use_power: Db<Power>,
    pub carrier_band: CarrierBand,

    pub reaction_time: Time,
}

impl From<ScenarioNodeSettings> for NodeSettings {
    fn from(value: ScenarioNodeSettings) -> Self {
        Self {
            sf: value.sf,
            bandwidth: value.bandwidth,
            clock_offset: Time::from_milis(0.0),
            max_power: value.max_power,
            use_power: value.max_power,
            carrier_band: value.carrier_band,
            reaction_time: value.reaction_time,
            coding_rate: value.coding_rate,
            is_gateway: value.is_gateway,
            movement_indicator: value.movement_indicator,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CarrierBand {
    B433,
    B868,
}

impl CarrierBand {
    /// The centre frequencies of the band for the default slot
    /// Taken from the [Meshtastic docs](https://meshtastic.org/docs/overview/radio-settings/#europe-frequency-bands)
    pub fn wave_length(self) -> Length {
        let x = match self {
            CarrierBand::B433 => 0.69096504,
            CarrierBand::B868 => 0.34477727,
        };

        Length::from_metres(x)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transmission {
    // Simulation Properties
    pub id: u32,
    pub transmitter_id: usize,

    // Timing
    pub start_time: Time,
    pub end_time: Time,

    // Physical Properties
    pub sf: i32,
    pub power: Db<Power>,
    pub carrier_band: CarrierBand,
    pub bandwidth: Frequency,

    //Packet Data
    pub header: Header,
    pub message_content: MessageContent,
}

impl Transmission {
    pub fn airtime(&self) -> Time {
        self.end_time - self.start_time
    }

    pub fn overlaps(&self, other: &Transmission) -> bool {
        self.start_time < other.end_time && other.start_time < self.end_time
    }
}

#[derive(Clone, Debug)]
pub struct NotifyStatus {
    pub notification: Option<Notification>,
    pub at_time: Time,
}

#[derive(Debug, Clone)]
pub struct MessageInfo {
    /// Size in bytes
    pub size: i32,

    /// Target node ids
    pub targets: Vec<usize>,
}

impl MessageInfo {
    pub fn std_destination(&self) -> Destination {
        if self.targets.len() == 1 {
            Destination::Node(*self.targets.first().expect("checked length"))
        } else {
            Destination::Broadcast
        }
    }
}

#[derive(Debug, Clone)]
pub struct SimEvent {
    pub time: Time,
    pub action: SimAction,
}

impl PartialEq for SimEvent {
    fn eq(&self, other: &Self) -> bool {
        self.time.eq(&other.time)
    }
}
impl Eq for SimEvent {}

impl PartialOrd for SimEvent {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.time.neg().partial_cmp(&other.time.neg())
    }
}

impl Ord for SimEvent {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.time.inner().neg().total_cmp(&other.time.inner().neg())
    }
}

#[derive(Debug, Clone)]
pub enum SimAction {
    GenerateMessage {
        node_id: usize,
        message_id: usize,
    },
    SendMessage {
        node_id: usize,
        header: Header,
        message_content: MessageContent,
    },
    RecieveMessage {
        node_id: usize,
        transmission_id: u32,
    },
    MaybeNotify {
        node_id: usize,
        on_thread: NodeThread,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogItem {
    pub time: Time,
    pub log_level: LogLevel,
    pub source: LogSource,
    pub content: LogContent,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum LogLevel {
    Error,
    Info,
    Debug,
    Trace,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum LogSource {
    Simulation,
    Node(usize),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogContent {
    Text(String),
    TransmissionSent {
        sender_id: usize,
        transmission_id: u32,
    },
    TransmissionReceived {
        receiver_id: usize,
        transmission_id: u32,
    },
    TransmissionBlocked {
        receiver_id: usize,
        target_transmission_id: u32,
        blocking_transmission_id: u32,
    },
}

impl Display for LogContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogContent::Text(text) => text.fmt(f),
            LogContent::TransmissionSent {
                sender_id,
                transmission_id,
            } => write!(
                f,
                "Transmission {} sent by node {}",
                transmission_id, sender_id
            ),
            LogContent::TransmissionReceived {
                receiver_id,
                transmission_id,
            } => write!(
                f,
                "Received transmission {} at node {}",
                transmission_id, receiver_id
            ),
            LogContent::TransmissionBlocked {
                receiver_id,
                target_transmission_id,
                blocking_transmission_id,
            } => write!(
                f,
                "Tranmission {} blocked at {} by at least {}",
                target_transmission_id, receiver_id, blocking_transmission_id,
            ),
        }
    }
}
