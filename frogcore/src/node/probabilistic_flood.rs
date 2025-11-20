use std::collections::HashSet;

use crate::{node::{meshtastic_header, MeshtasticHeader}, simulation::{data_structs::LogLevel, NodeError}};

use super::{
    meshtastic::MeshtasticRadioInterface, GlobalPacketId, ImplNodeModel,
    StoredPacket,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbabilisticFlood {
    seen: HashSet<GlobalPacketId>,
    radio_interface: MeshtasticRadioInterface<MeshtasticHeader>,
    next_packet_id: u32,
}

/// Number of hops before using probabalistic rebroadcasting
const MIN_HOPS: i32 = 2;

const REBROADCAST_PROB: f64 = 0.65;

impl ImplNodeModel for ProbabilisticFlood {
    type InnerHeader = MeshtasticHeader;
    fn identity_str(&self) -> &str {
        "Probabilistic Flood"
    }

    fn initalisation(&mut self, mut context: crate::simulation::Context) {
        self.radio_interface.on_initalisation(&mut context);
    }

    fn receive_message(
        &mut self,
        mut context: crate::simulation::Context,
        header: &Self::InnerHeader,
        message_content: crate::simulation::MessageContent,
        payload_size: i32,
        snr: crate::units::Db<f64>,
    ) {
        let packet = StoredPacket {
            header: header.clone(),
            message_content,
            size: payload_size,
            snr: Some(snr),
        };

        let key = packet.global_id();
        let node_id = context.node_id();

        if self.seen.contains(&key) {
            return;
        }

        if !packet.header.dest.is_to_node(context.node_id()) {
            let drop_packet: f64 =
                if (packet.header.hop_start - packet.header.hop_limit) >= MIN_HOPS {
                    context.rng(0.0, 1.0)
                } else {
                    0.0 // Always rebroadcast
                };

            if drop_packet < REBROADCAST_PROB {
                context.log(
                    || format!("Enqueuing rebroadcast for {key:?}"),
                    LogLevel::Info,
                );
                let mut rebroadcast_packet = packet.clone();
                rebroadcast_packet.header.hop_limit -= 1;
                self.radio_interface.send(&mut context, rebroadcast_packet);
            } else {
                context.log(
                    || format!("Probabilistically dropping rebroadcast for {key:?} at {node_id}"),
                    LogLevel::Info,
                );
            }
        }

        self.seen.insert(key);
    }

    fn generate_message(
        &mut self,
        mut context: crate::simulation::Context,
        message_id: crate::simulation::MessageContent,
        message_info: &crate::simulation::data_structs::MessageInfo,
    ) {
        let header = meshtastic_header(context.node_id(), self.next_packet_id(), message_info);

        let packet = StoredPacket {
            header: header,
            message_content: message_id.clone(),
            size: message_info.size,
            snr: None,
        };

        context.log(
            || {
                format!(
                    "Message {message_id:?} generated and enqueued as packet {:?}",
                    packet.global_id()
                )
            },
            LogLevel::Info,
        );

        self.radio_interface.send(&mut context, packet);
    }

    fn handle_error(
        &mut self,
        mut context: crate::simulation::Context,
        error: crate::simulation::NodeError,
    ) {
        match error {
            NodeError::RadioBusyError(_header, _content) => {
                context.log(|| "Radio busy error".into(), LogLevel::Error)
            }
        }
    }

    fn get_notified(
        &mut self,
        mut context: crate::simulation::Context,
        notification: super::Notification,
        thread: super::NodeThread,
    ) {
        self.radio_interface
            .on_get_notified(&mut context, notification, thread);
    }
}
impl Default for ProbabilisticFlood {
    fn default() -> Self {
        Self::new()
    }
}
impl ProbabilisticFlood {
    pub fn new() -> Self {
        ProbabilisticFlood {
            seen: HashSet::new(),
            radio_interface: MeshtasticRadioInterface::new(),
            next_packet_id: 0,
        }
    }

    fn next_packet_id(&mut self) -> u32 {
        let out = self.next_packet_id;
        self.next_packet_id += 1;
        out
    }
}
