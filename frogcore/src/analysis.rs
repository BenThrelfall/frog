use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{
    scenario::{MessageMarker, Scenario, ScenarioNodeSettings},
    sim_file::{OutputIdentity, SimOutput},
    simulation::{
        MessageContent,
        data_structs::{LogContent, LogItem, LogSource, Transmission},
    },
    units::{SECONDS, Time},
};

pub struct CompleteAnalysis {
    pub node_settings: Vec<ScenarioNodeSettings>,

    /// Lists is log items for each node.
    /// Outer vec is nodes (index is node id).
    pub node_events: Vec<Vec<LogItem>>,

    /// List of log items with [`LogSource::Simulation`]
    pub sim_events: Vec<LogItem>,

    //Event Type Breakdown (for sim events)
    pub transmission_sent_events: usize,
    pub transmission_received_events: usize,
    pub transmission_blocked_events: usize,

    /// List of transmissions ordered by start time.
    pub transmissions: Vec<Transmission>,

    /// Sum of the airtime of all transmissions in seconds.
    pub total_airtime: f64,

    /// Time of the last [`LogItem`] in [`Self::sim_events`] in seconds.
    pub end_time: f64,

    pub reception_analysis: ReceptionAnalysis,

    pub complete_identity: OutputIdentity,
}

impl CompleteAnalysis {
    pub fn new(results: SimOutput, scenario: Scenario) -> CompleteAnalysis {
        let node_settings = scenario.get_settings();
        let node_count = node_settings.len();

        let mut node_events = vec![Vec::new(); node_count];
        let mut sim_events = Vec::new();

        for event in results.logs {
            match event.source {
                LogSource::Simulation => sim_events.push(event),
                LogSource::Node(node_id) => node_events[node_id].push(event),
            }
        }

        sim_events.sort_by(|x, y| f64::total_cmp(&x.time.seconds(), &y.time.seconds()));

        let end_time = sim_events.last().map(|x| x.time.seconds()).unwrap_or(1.0);

        let (
            transmission_sent_events,
            transmission_received_events,
            transmission_blocked_events,
            text_events,
        ) = sim_events
            .iter()
            .fold((0, 0, 0, 0), |(a, b, c, d), event| match event.content {
                LogContent::TransmissionSent { .. } => (a + 1, b, c, d),
                LogContent::TransmissionReceived { .. } => (a, b + 1, c, d),
                LogContent::TransmissionBlocked { .. } => (a, b, c + 1, d),
                LogContent::Text(_) => (a, b, c, d + 1),
            });

        assert_eq!(
            transmission_sent_events
                + transmission_received_events
                + transmission_blocked_events
                + text_events,
            sim_events.len()
        );

        node_events.iter_mut().for_each(|list| {
            list.sort_by(|x, y| f64::total_cmp(&x.time.seconds(), &y.time.seconds()))
        });

        let mut transmissions = results.transmissions;
        transmissions
            .sort_by(|x, y| f64::total_cmp(&x.start_time.seconds(), &y.start_time.seconds()));

        let total_airtime = transmissions
            .iter()
            .map(|x| x.airtime().seconds())
            .sum::<f64>();

        let reception_analysis =
            ReceptionAnalysis::new(&scenario, &transmissions, &sim_events, node_count);

        let complete_identity = results.complete_identity;

        CompleteAnalysis {
            node_settings,
            node_events,
            sim_events,
            transmissions,
            end_time,
            reception_analysis,
            total_airtime,
            complete_identity,
            transmission_sent_events,
            transmission_received_events,
            transmission_blocked_events,
        }
    }
}

/// Collection of graphs by transmission id.
/// Each graph represents the sending node connected to each node that successfully recieved the transmission.
pub fn create_transmission_graphs(sim_events: Vec<LogItem>) -> HashMap<u32, TransmissionGraph> {
    let mut transmission_graphs = HashMap::new();

    for event in sim_events.iter() {
        if let LogContent::TransmissionSent {
            sender_id,
            transmission_id,
        } = event.content
        {
            let origin_transmission_id = transmission_id;

            let web = TransmissionGraph {
                origin: sender_id,
                targets: Vec::new(),
            };

            transmission_graphs.insert(origin_transmission_id, web);
        }
    }

    for event_prime in sim_events.iter() {
        if let LogContent::TransmissionReceived {
            receiver_id,
            transmission_id,
        } = event_prime.content
        {
            transmission_graphs
                .get_mut(&transmission_id)
                .unwrap()
                .targets
                .push(receiver_id);
        }
    }

    transmission_graphs
}

#[derive(Debug, Clone)]
pub struct WantedMessage {
    pub message_id: usize,
    pub was_received: bool,
    pub latency: Option<Time>,
}

pub struct ReceptionAnalysis {
    /// Lists of messages wanted by each node and if they were received.
    /// Inner item is message id and bool indicating reception.
    /// Outer vec is nodes (index is node id).
    pub wanted_messages: Vec<Vec<WantedMessage>>,

    /// List of all messages received by each node.
    /// Inner item is the message id.
    /// Outer vec is nodes (index is node id).
    pub received_messages: Vec<Vec<usize>>,

    /// Average time between generation and reception over all received wanted message
    /// at each node. Messages that are never received do not effect this value.
    pub avg_latency_per_node: Vec<Time>,

    pub avg_avg_latency: Time,
    pub min_avg_latency: Time,
    pub max_avg_latency: Time,

    pub global_latency: Time,

    pub l120_score: Time,
    pub l600_score: Time,
    pub l6000_score: Time,

    pub t120_reception: f64,
    pub t600_reception: f64,
    pub t1800_reception: f64,
    pub t6000_reception: f64,

    /// If the scenario had an emergency,
    /// how long did it take between the first emergency packet being sent
    /// and an emergency packet arriving at the gateway.
    ///
    /// Assumes that all emergency packets are for a single emergency.
    pub emergency_result: EmergencyResult,

    /// Proportion of recieved packets that contain new messages.
    /// Includes non-message packets such as Acks and Naks.
    pub all_packet_uniqueness: f64,

    /// Proportion of recieved packets that contain new messages.
    /// Only includes message packets.
    pub message_packet_uniqueness: f64,

    pub phantom_uniqueness: f64,

    /// List of reception rate of wanted messages at each node.
    /// Reception rate is `total_wanted_messages / received_wanted_messages`.
    /// Index is node id.
    pub reception_rate: Vec<f64>,

    pub global_reception_rate: f64,

    pub average_reception_rate: f64,
    pub max_reception_rate: f64,
    pub min_reception_rate: f64,

    pub message_reception_directness: f64,
    pub reception_directness: f64,

    pub message_reception_unique_directness: f64,
    pub reception_unique_directness: f64,

    pub message_transmission_directness: f64,
    pub transmission_directness: f64,

    pub message_transmission_unique_directness: f64,
    pub transmission_unique_directness: f64,

    pub gateway_reception: f64,
    pub gateway_latency: Time,
}

impl ReceptionAnalysis {
    fn new(
        scenario: &Scenario,
        transmissions: &Vec<Transmission>,
        sim_events: &Vec<LogItem>,
        node_count: usize,
    ) -> ReceptionAnalysis {
        let mut wanted_messages = vec![Vec::new(); node_count];
        let mut received_messages = vec![HashSet::new(); node_count];

        let mut latency_per_node: Vec<HashMap<usize, Time>> = vec![HashMap::new(); node_count];
        let mut foobar_per_node: Vec<HashMap<usize, u32>> = vec![HashMap::new(); node_count];

        let maybe_max_id = transmissions.iter().map(|x| x.id).max();

        let id_to_index = if let Some(max_id) = maybe_max_id {
            let mut data = vec![0; max_id as usize + 1];
            transmissions
                .iter()
                .enumerate()
                .for_each(|(index, x)| data[x.id as usize] = index);
            data
        } else {
            vec![]
        };

        for event in sim_events.iter() {
            let LogContent::TransmissionReceived {
                receiver_id,
                transmission_id,
            } = event.content
            else {
                continue;
            };

            let transmission = &transmissions[id_to_index[transmission_id as usize]];

            if let MessageContent::GeneratedMessage(id) = transmission.message_content {
                received_messages[receiver_id].insert(id);
                let prev = latency_per_node[receiver_id].get(&id);
                let this_latency = transmission.end_time - scenario.messages[id].generate_time;

                if let Some(&latency) = prev {
                    if this_latency < latency {
                        latency_per_node[receiver_id].insert(id, this_latency);
                        foobar_per_node[receiver_id].insert(id, transmission_id);
                    }
                } else {
                    latency_per_node[receiver_id].insert(id, this_latency);
                    foobar_per_node[receiver_id].insert(id, transmission_id);
                }
            }
        }

        for (i, message) in scenario.messages.iter().enumerate() {
            message.targets.iter().for_each(|&x| {
                wanted_messages[x].push(WantedMessage {
                    message_id: i,
                    was_received: received_messages[x].contains(&i),
                    latency: latency_per_node[x].get(&i).copied(),
                });
            });
        }

        // Latency Score / Penalised Latency

        let l120_score = latency_score(&wanted_messages, 120.0 * SECONDS);
        let l600_score = latency_score(&wanted_messages, 600.0 * SECONDS);
        let l6000_score = latency_score(&wanted_messages, 6000.0 * SECONDS);

        // Latency Thresholded Reception

        let t120_reception = threshold_reception(&wanted_messages, 120.0 * SECONDS);
        let t600_reception = threshold_reception(&wanted_messages, 600.0 * SECONDS);
        let t1800_reception = threshold_reception(&wanted_messages, 1800.0 * SECONDS);
        let t6000_reception = threshold_reception(&wanted_messages, 6000.0 * SECONDS);

        // Packet Uniqueness

        let mut non_message_receptions = 0.0;
        let mut message_receptions = 0.0;
        let mut blocked_receptions = 0.0;

        sim_events.iter().for_each(|x| match x.content {
            LogContent::TransmissionReceived {
                transmission_id, ..
            } => {
                let transmission = &transmissions[id_to_index[transmission_id as usize]];
                match transmission.message_content {
                    MessageContent::GeneratedMessage(_) => message_receptions += 1.0,
                    _ => non_message_receptions += 1.0,
                }
            }
            LogContent::TransmissionBlocked { .. } => blocked_receptions += 1.0,
            _ => (),
        });

        let unique_receptions = received_messages.iter().map(|x| x.len()).sum::<usize>() as f64;

        let all_packet_uniqueness =
            unique_receptions / (message_receptions + non_message_receptions);
        let message_packet_uniqueness = unique_receptions / message_receptions;
        let phantom_uniqueness = unique_receptions / (message_receptions + blocked_receptions);

        // Packet Directness

        let direct_receptions = sim_events
            .iter()
            .filter(|x| match x.content {
                LogContent::TransmissionReceived {
                    receiver_id,
                    transmission_id,
                } => {
                    let transmission = &transmissions[id_to_index[transmission_id as usize]];
                    match transmission.message_content {
                        MessageContent::GeneratedMessage(message_id) => wanted_messages
                            [receiver_id]
                            .iter()
                            .any(|x| x.message_id == message_id),
                        _ => false,
                    }
                }
                _ => false,
            })
            .count() as f64;

        let direct_unique_receptions = wanted_messages
            .iter()
            .map(|x| x.iter().filter(|y| y.was_received).count())
            .sum::<usize>() as f64;

        let message_reception_directness = direct_receptions / message_receptions;
        let message_reception_unique_directness = direct_unique_receptions / message_receptions;
        let reception_directness =
            direct_receptions / (message_receptions + non_message_receptions);
        let reception_unique_directness =
            direct_unique_receptions / (message_receptions + non_message_receptions);

        // We call a transmission direct if at least one node that wants the message it carries recieves it

        let mut message_transmissions = vec![false; transmissions.len()];
        let mut direct_transmissions = vec![false; transmissions.len()];
        let mut green_direct_transmissions = vec![false; transmissions.len()];

        for event in sim_events.iter() {
            let LogContent::TransmissionReceived {
                receiver_id,
                transmission_id,
            } = event.content
            else {
                continue;
            };

            let transmission = &transmissions[id_to_index[transmission_id as usize]];

            let MessageContent::GeneratedMessage(message_id) = transmission.message_content else {
                continue;
            };

            message_transmissions[transmission_id as usize] = true;

            if wanted_messages[receiver_id]
                .iter()
                .any(|x| x.message_id == message_id)
            {
                direct_transmissions[transmission_id as usize] = true;

                if foobar_per_node[receiver_id][&message_id] == transmission_id {
                    green_direct_transmissions[transmission_id as usize] = true;
                }
            }
        }

        let message_transmissions_len = message_transmissions.iter().filter(|x| **x).count() as f64;

        let transmission_directness =
            direct_transmissions.iter().filter(|x| **x).count() as f64 / transmissions.len() as f64;
        let transmission_unique_directness =
            green_direct_transmissions.iter().filter(|x| **x).count() as f64
                / transmissions.len() as f64;

        let message_transmission_directness =
            direct_transmissions.iter().filter(|x| **x).count() as f64 / message_transmissions_len;
        let message_transmission_unique_directness =
            green_direct_transmissions.iter().filter(|x| **x).count() as f64
                / message_transmissions_len;

        // Emergency Analysis

        let maybe_emergency_start = scenario
            .messages
            .iter()
            .filter(|x| x.markers.contains(&MessageMarker::Emergency))
            .map(|x| x.generate_time)
            .min_by(|x, y| x.partial_cmp(&y).unwrap());

        let emergency_result = if let Some(emergency_start) = maybe_emergency_start {
            let gateway_emer_recptions = sim_events.iter().filter_map(|event| {
                let LogContent::TransmissionReceived {
                    receiver_id,
                    transmission_id,
                } = event.content
                else {
                    return None;
                };

                if !scenario.get_settings()[receiver_id].is_gateway {
                    return None;
                }

                let transmission = &transmissions[id_to_index[transmission_id as usize]];

                let MessageContent::GeneratedMessage(message_id) = transmission.message_content
                else {
                    return None;
                };

                if !scenario.messages[message_id]
                    .markers
                    .contains(&MessageMarker::Emergency)
                {
                    return None;
                }

                return Some(transmission.end_time);
            });

            let maybe_arrival_time =
                gateway_emer_recptions.min_by(|x, y| x.partial_cmp(&y).unwrap());

            match maybe_arrival_time {
                Some(arrival_time) => EmergencyResult::Latency(arrival_time - emergency_start),
                None => EmergencyResult::NotRecieved,
            }
        } else {
            EmergencyResult::NotEmergency
        };

        // global reception and latency

        let global_latency = {
            let (agg, total) = wanted_messages
                .iter()
                .flat_map(|messages| messages.iter().filter_map(|x| x.latency))
                .fold((0.0 * SECONDS, 0), |(agg, total), val| {
                    (agg + val, total + 1)
                });

            agg / (total as f64).max(1.0)
        };

        let global_reception_rate = {
            let total: usize = wanted_messages.iter().map(|x| x.len()).sum();
            let agg: usize = wanted_messages
                .iter()
                .map(|messages| messages.iter().filter(|x| x.was_received).count())
                .sum();

            (agg as f64) / (total as f64).max(1.0)
        };

        // Gateway reception and latency

        let gateway_latency = {
            let (agg, total) = wanted_messages
                .iter()
                .enumerate()
                .filter(|(id, _)| scenario.settings[*id].is_gateway)
                .flat_map(|(_, messages)| messages.iter().filter_map(|x| x.latency))
                .fold((0.0 * SECONDS, 0), |(agg, total), val| {
                    (agg + val, total + 1)
                });

            agg / (total as f64).max(1.0)
        };

        let gateway_reception = {
            let mut agg = 0;
            let mut total = 0;
            wanted_messages
                .iter()
                .enumerate()
                .filter(|(id, _)| scenario.settings[*id].is_gateway)
                .map(|(_, messages)| {
                    (
                        messages.iter().filter(|x| x.was_received).count(),
                        messages.len(),
                    )
                })
                .for_each(|(val, len)| {
                    agg += val;
                    total += len;
                });

            (agg as f64) / (total as f64).max(1.0)
        };

        // mins, maxes and averages
        let avg_latency_per_node: Vec<Time> = wanted_messages
            .iter()
            .map(|mes_list| {
                let (sum, count) = mes_list
                    .iter()
                    .filter_map(|message| message.latency)
                    .fold((Time::from_seconds(0.0), 0), |(sum, count), val| {
                        (sum + val, count + 1)
                    });

                sum / (count as f64).max(1.0)
            })
            .collect();

        let avg_avg_latency =
            avg_latency_per_node.iter().copied().sum::<Time>() / (node_count as f64);

        let min_avg_latency = avg_latency_per_node
            .iter()
            .copied()
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();

        let max_avg_latency = avg_latency_per_node
            .iter()
            .copied()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();

        let mut reception_rate = vec![0.0; node_count];

        for (node_id, inner) in wanted_messages.iter().enumerate() {
            let mut received = 0.0;
            let mut total = 0.0;
            for wanted in inner {
                total += 1.0;
                if wanted.was_received {
                    received += 1.0;
                }
            }

            if total == 0.0 {
                reception_rate[node_id] = 1.0;
            } else {
                reception_rate[node_id] = received / total;
            }
        }

        let mut received_messages: Vec<Vec<usize>> = received_messages
            .into_iter()
            .map(|x| x.into_iter().collect())
            .collect();

        received_messages.iter_mut().for_each(|x| x.sort());

        let average_reception_rate =
            reception_rate.iter().sum::<f64>() / (reception_rate.len() as f64);
        let min_reception_rate = reception_rate
            .iter()
            .copied()
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();
        let max_reception_rate = reception_rate
            .iter()
            .copied()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();

        ReceptionAnalysis {
            wanted_messages,
            received_messages,
            reception_rate,
            average_reception_rate,
            max_reception_rate,
            min_reception_rate,
            avg_latency_per_node,
            avg_avg_latency,
            min_avg_latency,
            max_avg_latency,
            l120_score,
            l600_score,
            l6000_score,
            all_packet_uniqueness,
            phantom_uniqueness,
            message_packet_uniqueness,
            message_reception_directness,
            reception_directness,
            message_reception_unique_directness,
            reception_unique_directness,
            message_transmission_directness,
            transmission_directness,
            message_transmission_unique_directness,
            transmission_unique_directness,
            emergency_result,
            global_latency,
            t120_reception,
            t600_reception,
            t1800_reception,
            t6000_reception,
            global_reception_rate,
            gateway_reception,
            gateway_latency,
        }
    }
}

fn latency_score(wanted_messages: &Vec<Vec<WantedMessage>>, penalty_time: Time) -> Time {
    let node_count = wanted_messages.len();

    // Currently averages the averages rather than averaging the aggregated
    // values over the total number of message wants. Maybe this should change

    wanted_messages
        .iter()
        .map(|mes_list| {
            mes_list
                .iter()
                .map(|message| message.latency.unwrap_or(penalty_time).min(penalty_time))
                .fold(Time::from_seconds(0.0), |a, val| {
                    a + (val / mes_list.len() as f64)
                })
        })
        .sum::<Time>()
        / (node_count as f64)
}

fn threshold_reception(wanted_messages: &Vec<Vec<WantedMessage>>, threshold: Time) -> f64 {
    let total: usize = wanted_messages.iter().map(|x| x.len()).sum();

    let ni: usize = wanted_messages
        .iter()
        .map(|mes_list| {
            mes_list
                .iter()
                .filter(|message| message.latency.is_some_and(|lat| lat <= threshold))
                .count()
        })
        .sum();

    ni as f64 / total as f64
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum EmergencyResult {
    NotEmergency,
    NotRecieved,
    Latency(Time),
}

#[derive(Debug, Clone)]
pub struct TransmissionGraph {
    pub origin: usize,
    pub targets: Vec<usize>,
}
