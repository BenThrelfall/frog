use std::{
    cell::RefCell,
    collections::{BinaryHeap, HashMap},
    i32,
};

use crate::{
    node::NodeModel,
    node_location::{NodeLocation, Point},
    scenario::{Scenario, ScenarioMessage},
    sim_file::{OutputIdentity, SimOutput},
    units::{Db, Frequency, Power},
};

use data_structs::{
    LogContent, LogItem, LogLevel, LogSource, MessageInfo, NodeSettings, NotifyStatus, SimAction,
    SimEvent, Transmission,
};
use models::{TransmissionModel, TransmissionResult};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha12Rng;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    Time,
    node::{CustomContent, Header, ImplNodeModel, NodeThread, Notification},
};

pub mod data_structs;
mod em;
pub mod models;

type EventQueue = BinaryHeap<SimEvent>;

const SIM_END: Time = Time::from_seconds(60.0 * 60.0 * 4.0); //Time::from_imilis(i32::MAX / 2);

pub fn run_simulation(
    random_seed: u64,
    scenario: Scenario,
    model: NodeModel,
    do_node_logs: bool,
) -> SimOutput {
    let scenario_identity = scenario.identity.clone();

    let mut sim = init_simulation(random_seed, scenario, model, do_node_logs);

    while !sim.finished() {
        sim.step();
    }

    let version = "0.1.0";
    SimOutput {
        complete_identity: OutputIdentity {
            scenario_identity,
            model_id: model_identity_string(&sim.node_identities()),
            simulation_seed: random_seed,
            sim_version: version.to_string(),
        },
        logs: sim.logs,
        transmissions: sim.em_field,
    }
}

fn init_simulation(
    random_seed: u64,
    scenario: Scenario,
    model: NodeModel,
    do_node_logs: bool,
) -> Simulation {
    let node_settings = scenario.get_settings();

    // Set up Simulation and create node structs
    let mut sim = Simulation::new(
        scenario.map,
        node_settings.into_iter().map(|x| x.into()),
        scenario.model,
        random_seed,
        model,
        do_node_logs,
    );

    // Add message generation to event queue
    sim.enqueue_message_generation(scenario.messages.iter().cloned());

    // Call node init
    sim.initalise_nodes();

    sim
}

fn model_identity_string(models: &Vec<String>) -> String {
    let first = models.first().unwrap();
    let all_same = models.iter().all(|x| x == first);

    if all_same {
        first.clone()
    } else {
        models.join("; ")
    }
}

/// Provides access to the underlying simulation to a node.
/// [Context] is structured such that only information and operations
/// that are physically available to the node accessed.
/// This should hopefully make it easier to write realistic node models.
pub struct Context<'a> {
    events: &'a mut EventQueue,
    sim_time: Time,
    node_id: usize,
    notify_status: &'a mut HashMap<NodeThread, NotifyStatus>,
    logs: &'a mut Vec<LogItem>,
    settings: &'a mut NodeSettings,
    rng: &'a RefCell<ChaCha12Rng>,
    transmission: &'a TransmissionModel,
    em_field: &'a Vec<Transmission>,
    graph: &'a NodeLocation,
    do_node_logs: bool,
}

pub enum NodeError {
    RadioBusyError(Header, MessageContent),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageContent {
    GeneratedMessage(usize),
    NodeMessage(CustomContent),
    Empty,
}

#[derive(Debug, Error)]
#[error("Provided value was out of range")]
pub struct NodeUpdateError;

impl<'a> Context<'a> {
    /// Returns the clock time of the current node
    pub fn clock_time(&self) -> Time {
        self.sim_time + self.settings.clock_offset
    }

    /// Try and get the nodes current location.
    /// You can imagine this like an interface to GPS or similar.
    pub fn location(&self) -> Option<Point> {
        self.graph.location(self.sim_time, self.node_id)
    }

    /// Returns the node id of the current node
    pub fn node_id(&self) -> usize {
        self.node_id
    }

    /// Returns the node settings of the current node
    pub fn node_setting(&self) -> &NodeSettings {
        self.settings
    }

    pub fn change_sf(&mut self, sf: i32) -> Result<(), NodeUpdateError> {
        if sf < 7 || sf > 12 {
            return Err(NodeUpdateError);
        }

        self.settings.sf = sf;
        Ok(())
    }

    pub fn change_coding_rate(&mut self, coding_rate: i32) -> Result<(), NodeUpdateError> {
        if coding_rate < 4 {
            return Err(NodeUpdateError);
        }

        self.settings.coding_rate = coding_rate;
        Ok(())
    }

    pub fn change_bandwidth(&mut self, bandwidth: Frequency) {
        self.settings.bandwidth = bandwidth;
    }

    pub fn change_power(&mut self, use_power: Db<Power>) -> Result<(), NodeUpdateError> {
        if use_power > self.settings.max_power {
            return Err(NodeUpdateError);
        }

        self.settings.use_power = use_power;
        Ok(())
    }

    /// Used for transmitting messages in the simulation.
    ///
    /// Enqueues a send event that will be processed with some delay depending on the nodes [`NodeSettings::reaction_time`].
    /// When the event is executed the message will be broadcast
    /// or a [NodeError::RadioBusyError] will be raised if the node was already broadcasting.
    /// Consider checking if the radio is free before calling this.
    ///
    /// Once transmission is complete, other nodes that successfully receive the message will get the transmitted `header`
    /// and `message_content` in a [`NodeModel::receive_message`] call.
    ///
    /// - `header` - The header portion of the message. This is currently the same for all messages and accross all node models
    /// but that may change in the future.
    /// - `message_content` - The content of the message. For a message from a simulated user this should be the assossiated
    /// [`MessageContent::GeneratedMessage`] otherwise it will be a custom message. Custom messages are used for the node models
    /// own purposes, likely as part of a routing algorithm.
    pub fn enqueue_send(&mut self, header: impl Into<Header>, message_content: MessageContent) {
        self.events.push(SimEvent {
            time: self.sim_time + self.settings.reaction_time,
            action: SimAction::SendMessage {
                node_id: self.node_id,
                message_content,
                header: header.into(),
            },
        });
    }

    /// Logs an event in the simulation logs. This event is automatically associated with the current node.
    pub fn log(&mut self, text: impl FnOnce() -> String, level: LogLevel) {
        if self.do_node_logs {
            self.logs.push(LogItem {
                time: self.sim_time,
                log_level: level,
                source: LogSource::Node(self.node_id),
                content: LogContent::Text(text()),
            });
        }
    }

    /// Register a thread for use with [Self::notify_later].
    /// This should be called exactly once for each thread the node model uses.
    /// Usually this should only be called in [NodeModel::initalisation].
    pub fn register_thread(&mut self, thread: NodeThread) {
        self.notify_status.insert(
            thread,
            NotifyStatus {
                notification: None,
                at_time: Time::from_seconds(0.0),
            },
        );
    }

    /// Register a notification to be raise on the current node at a later time.
    /// Works like a simulated meshtastic notified worker thread on the node.
    /// If `should_override` is false and there is a notification pending for the specified thread
    /// nothing will happen. Otherwise the notification is registered for the specified thread and
    /// at the provided time `NodeModel::get_notified` will be called for the given thread and notification.
    pub fn notify_later(
        &mut self,
        delay: Time,
        notif: Notification,
        thread: NodeThread,
        should_override: bool,
    ) {
        let notify_status = self.notify_status.get_mut(&thread).unwrap();

        if should_override
            || notify_status.at_time < self.sim_time
            || notify_status.notification.is_none()
        {
            let notify_time = self.sim_time + delay;
            notify_status.notification = Some(notif);
            notify_status.at_time = notify_time;

            self.events.push(SimEvent {
                time: notify_time,
                action: SimAction::MaybeNotify {
                    node_id: self.node_id,
                    on_thread: thread,
                },
            });
        }
    }

    /// Is the current node currently transmitting
    pub fn is_transmitting(&self) -> bool {
        self.active_transmissions()
            .find(|x| x.transmitter_id == self.node_id)
            .is_some()
    }

    /// Is the channel in use based on what the current node can observe
    pub fn channel_in_use(&self) -> bool {
        self.transmission.detecting_any_at(self, self.node_id)
    }

    /// Returns the current nodes calculation of the channel utilisation
    /// based on what the node has observed and the algorithm from airtime.cpp
    pub fn channel_utilisation(&self) -> f64 {
        self.observed_utalisation(self.node_id)
    }

    /// Generate a random float between the min and max (inclusive..exclusive)
    /// This method should always be used for creating random values in node models
    pub fn rng(&mut self, min: f64, max: f64) -> f64 {
        self.rng.borrow_mut().random_range(min..max)
    }

    /// Returns proportion channel utalisation (between 0.0 and 1.0)
    /// based on what proportion of the time the node could detect activity on the channel
    /// or was transmitting. Full window that is considered is 60 seconds but rolling 10 second
    /// buckets are used meaning on average the utalisation over the last 55 seconds is being returned.
    /// Based on airtime.cpp in meshtastic firmware.
    ///
    /// Currently detections are counted even if the decoding was blocked by interference
    /// but there is some chance this not how the real firmware works. See RadioLibInterface.cpp:handleReceiveInterupt
    fn observed_utalisation(&self, at_node: usize) -> f64 {
        // Constants taken from airtime.cpp
        const CHANNEL_UTILIZATION_PERIODS: i32 = 6;
        const UTIL_PERIOD_LENGTH: Time = Time::from_seconds(10.0);

        // utalisation is calculated with rolling discrete periods that reset to 0 on rollover
        // so the most recent period will only be partly filled (time wise)
        let full_periods = (CHANNEL_UTILIZATION_PERIODS - 1) as f64;
        let look_back_time = full_periods * UTIL_PERIOD_LENGTH + self.sim_time % UTIL_PERIOD_LENGTH;

        let limit_time = self.sim_time - look_back_time;

        let mut end_clamp = self.sim_time;
        let start_clamp = limit_time;
        let mut total = Time::from_seconds(0.0);

        let observation_range = self
            .em_field
            .iter()
            .rev()
            .take_while(|x| x.end_time >= limit_time)
            .filter(|x| self.transmission.detected_at(self, at_node, x));

        for x in observation_range {
            if x.start_time < end_clamp {
                total = total + x.end_time.min(end_clamp) - x.start_time.max(start_clamp);
                end_clamp = x.start_time;

                if end_clamp < start_clamp {
                    break;
                }
            }
        }

        let out = total / look_back_time;

        // TEST
        assert!(out <= 1.00001 && out >= 0.0, "value was {}", out);

        out
    }

    pub(super) fn active_transmissions(&self) -> impl Iterator<Item = &Transmission> {
        self.em_field
            .iter()
            .rev()
            .take_while(|x| x.end_time >= self.sim_time)
    }
}

#[derive(Debug, Clone)]
pub struct Simulation {
    pub sim_time: Time,
    event_queue: EventQueue,
    graph: NodeLocation,
    nodes: Vec<NodeModel>,
    node_settings: Vec<NodeSettings>,
    notify_status: Vec<HashMap<NodeThread, NotifyStatus>>,
    pub em_field: Vec<Transmission>,
    next_trans_id: u32,

    test_messages: Vec<MessageInfo>,

    pub logs: Vec<LogItem>,

    // Output Detail
    do_node_logs: bool,

    // Models
    transmission: TransmissionModel,
    rng: RefCell<ChaCha12Rng>,
}

/// Used to create a Context object.
/// Pass the simulation and the node id of the node the context is for.
/// `let context = context!(self, node_id);`
#[macro_export]
macro_rules! context {
    ($sim: expr, $node_id: expr) => {{
        Context {
            events: &mut $sim.event_queue,
            sim_time: $sim.sim_time,
            node_id: $node_id,
            notify_status: &mut $sim.notify_status[$node_id],
            settings: &mut $sim.node_settings[$node_id],
            logs: &mut $sim.logs,
            em_field: &$sim.em_field,
            graph: &$sim.graph,
            transmission: &$sim.transmission,
            rng: &$sim.rng,
            do_node_logs: $sim.do_node_logs,
        }
    }};
}

impl Simulation {
    pub fn new(
        graph: NodeLocation,
        node_settings: impl Iterator<Item = NodeSettings>,
        transmission: TransmissionModel,
        random_seed: u64,
        node_model: NodeModel,
        do_node_logs: bool,
    ) -> Self {
        let graph_len = graph.len();

        let sim = Simulation {
            sim_time: 0.0.into(),
            event_queue: BinaryHeap::new(),
            graph,
            em_field: Vec::new(),
            nodes: (0..graph_len).map(|_| node_model.clone()).collect(),
            node_settings: node_settings.take(graph_len).collect(),
            notify_status: (0..graph_len).map(|_| HashMap::new()).collect(),
            test_messages: Vec::new(),
            next_trans_id: 0,
            transmission,
            logs: Vec::new(),
            rng: ChaCha12Rng::seed_from_u64(random_seed).into(),
            do_node_logs,
        };

        sim
    }

    /// Returns true if there are no more events to process
    /// (meaning the simulation is complete) false otherwise.
    pub fn finished(&self) -> bool {
        self.event_queue.len() == 0
    }

    pub fn initalise_nodes(&mut self) {
        self.nodes.iter_mut().enumerate().for_each(|(id, node)| {
            let context = context!(self, id);
            node.initalisation(context);
        });
    }

    pub fn step(&mut self) {
        let Some(event) = self.event_queue.pop() else {
            return;
        };

        self.sim_time = event.time;

        if self.sim_time >= SIM_END {
            self.event_queue.drain().for_each(|x| match x.action {
                SimAction::MaybeNotify { .. } => (),
                _ => (), //eprintln!("!! Non-notify was in event queue at end !!"),
            });

            return;
        }

        let action = event.action;

        match action {
            SimAction::GenerateMessage {
                node_id,
                message_id,
            } => {
                let context = context!(self, node_id);

                let message_info = &self.test_messages[message_id];

                self.nodes[node_id].generate_message(
                    context,
                    MessageContent::GeneratedMessage(message_id),
                    message_info,
                );
            }
            SimAction::RecieveMessage {
                node_id,
                transmission_id,
            } => {
                let this_trans = self
                    .em_field
                    .iter()
                    .rev()
                    .find(|x| x.id == transmission_id)
                    .unwrap();

                let context = context!(self, node_id);
                let trans_res = self
                    .transmission
                    .reception_at(&context, node_id, this_trans);

                let snr = match trans_res {
                    TransmissionResult::Blocked { blocker_id } => {
                        self.log_content(
                            LogContent::TransmissionBlocked {
                                receiver_id: node_id,
                                target_transmission_id: this_trans.id,
                                blocking_transmission_id: blocker_id,
                            },
                            LogLevel::Debug,
                        );
                        return;
                    }
                    TransmissionResult::TooWeak => return,
                    TransmissionResult::Success { snr } => snr,
                };

                let message_size = self.message_size(&this_trans.message_content);

                let context = context!(self, node_id);

                self.nodes[node_id].receive_message(
                    context,
                    &this_trans.header,
                    this_trans.message_content.clone(),
                    message_size,
                    snr,
                );

                self.log_content(
                    LogContent::TransmissionReceived {
                        receiver_id: node_id,
                        transmission_id: this_trans.id,
                    },
                    LogLevel::Info,
                );
            }
            SimAction::SendMessage {
                node_id,
                header,
                message_content,
            } => {
                self.try_broadcast(node_id, header, message_content);
            }
            SimAction::MaybeNotify { node_id, on_thread } => {
                let status = self.notify_status[node_id]
                    .get_mut(&on_thread)
                    .expect("existed when this action was created");

                if status.at_time == self.sim_time {
                    if let Some(notif) = status.notification {
                        // Remove notification
                        status.notification = None;

                        let context = context!(self, node_id);
                        self.nodes[node_id].get_notified(context, notif, on_thread);
                    }
                }
            }
        }
    }

    pub fn enqueue_message_generation(&mut self, messages: impl Iterator<Item = ScenarioMessage>) {
        messages.for_each(|x| {
            let message_id = self.test_messages.len();
            self.test_messages.push(MessageInfo {
                size: x.size,
                targets: x.targets,
            });

            for generation in 0..x.num_generations {
                self.event_queue.push(SimEvent {
                    time: x.generate_time + x.generation_spacing * generation as f64,
                    action: SimAction::GenerateMessage {
                        node_id: x.sender,
                        message_id,
                    },
                });
            }
        });
    }

    pub fn node_identities(&self) -> Vec<String> {
        self.nodes
            .iter()
            .map(|x| x.identity_str().to_owned())
            .collect()
    }

    #[allow(dead_code)]
    fn log(&mut self, text: String, level: LogLevel) {
        self.logs.push(LogItem {
            time: self.sim_time,
            log_level: level,
            source: LogSource::Simulation,
            content: LogContent::Text(text),
        });
    }

    fn log_content(&mut self, content: LogContent, level: LogLevel) {
        self.logs.push(LogItem {
            time: self.sim_time,
            log_level: level,
            source: LogSource::Simulation,
            content: content,
        });
    }
}

#[derive(Debug, Clone)]
pub struct LiveSimulation {
    active: Simulation,
    base: Simulation,
}

impl LiveSimulation {
    pub fn new(
        random_seed: u64,
        scenario: Scenario,
        model: NodeModel,
        do_node_logs: bool,
    ) -> LiveSimulation {
        let sim = init_simulation(random_seed, scenario, model, do_node_logs);

        LiveSimulation {
            active: sim.clone(),
            base: sim,
        }
    }

    pub fn inspect_node(&mut self, node_id: usize, at_time: Time) -> &NodeModel {
        if at_time < self.active.sim_time {
            self.active = self.base.clone();
        }

        while self
            .active
            .event_queue
            .peek()
            .map(|x| x.time <= at_time)
            .unwrap_or(false)
        {
            self.active.step();
        }

        &self.active.nodes[node_id]
    }
}
