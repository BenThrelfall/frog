use rand::{seq::IndexedRandom, Rng};
use rand_chacha::ChaCha12Rng;
use rand_distr::Normal;
use serde::{Deserialize, Serialize};

use crate::{
    scenario::{ScenarioMessage, ScenarioNodeSettings},
    units::*,
};

/// Messages distributed independent of each other.
///
/// `gateway_priority = 0` means uniform across nodes.
/// `gateway_priority != 0` means more messages generated for gateways.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndependentRandomMessaging {
    pub message_count: usize,
    /// Messages will be uniformly randomly distributed across this time period
    pub messaging_timespan: Time,
    /// mean before applying clamping and rounding
    pub mean_message_size: f64,
    /// standard deviation before applying clamping and rounding
    pub std_message_size: f64,
    /// between 0.0 and 1.0
    /// Each message will either be a dm or broadcast with this chance
    pub broadcast_chance: f64,

    /// Proportion of messages generated from a gateway.
    pub gateway_priority: f64,
}

impl IndependentRandomMessaging {
    pub(super) fn generate(
        &self,
        nodes: &[ScenarioNodeSettings],
        rng: &mut ChaCha12Rng,
    ) -> Vec<ScenarioMessage> {
        let IndependentRandomMessaging {
            message_count,
            messaging_timespan,
            mean_message_size,
            std_message_size,
            broadcast_chance,
            gateway_priority,
        } = self.clone();

        let mut message_times: Vec<_> = (0..message_count)
            .map(|_| messaging_timespan.map(|x| rng.random_range(0.0..x)))
            .collect();

        message_times.sort_by(|a, b| a.partial_cmp(b).expect("Shoud not be NaN"));

        let message_size_dist = Normal::new(mean_message_size, std_message_size).unwrap();

        let node_count = nodes.len();
        let gateways: Vec<_> = nodes
            .iter()
            .enumerate()
            .filter_map(|(n, x)| x.is_gateway.then_some(n))
            .collect();

        message_times
            .iter()
            .map(|t| {
                let sender = if rng.random_bool(gateway_priority) {
                    gateways
                        .choose(rng)
                        .copied()
                        .expect("Should be gateways if gateway_priority != 0")
                } else {
                    rng.random_range(0..node_count)
                };

                ScenarioMessage::new(
                    sender,
                    message_targets(node_count, sender, broadcast_chance, rng),
                    *t,
                    rng.sample(message_size_dist).clamp(1.0, 237.0).round() as i32,
                )
            })
            .collect()
    }
}

fn message_targets(
    node_count: usize,
    sender: usize,
    broadcast_chance: f64,
    rng: &mut ChaCha12Rng,
) -> Vec<usize> {
    if rng.random_bool(broadcast_chance) {
        (0..node_count)
            .filter(|&x| x != sender)
            .collect()
    } else {
        let target = loop {
            let send_to = rng.random_range(0..node_count);

            if send_to != sender {
                break send_to;
            }
        };

        vec![target]
    }
}