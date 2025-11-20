use std::collections::{HashSet, VecDeque};

use crate::{
    node::{basic_header, BasicHeader, Destination, NodeThread},
    simulation::{self, data_structs::LogLevel, Context, MessageContent, NodeError},
    units::Time,
};

use super::{
    meshtastic::MeshtasticRadioInterface, CustomContent, GlobalPacketId, ImplNodeModel,
    Notification, StoredPacket,
};
use serde::{Deserialize, Serialize};

const MAX_REBROADCASTS: i32 = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcknowledgedOrRepeatFlood {
    rebroadcasts: VecDeque<(StoredPacket<BasicHeader>, i32)>,
    acknowledgements: HashSet<GlobalPacketId>,
    radio_interface: MeshtasticRadioInterface<BasicHeader>,
    next_packet_id: u32,
}

impl ImplNodeModel for AcknowledgedOrRepeatFlood {
    type InnerHeader = BasicHeader;

    fn identity_str(&self) -> &str {
        "Acknowledged Or Repeat Flood 1.1"
    }

    fn initalisation(&mut self, mut context: Context) {
        self.radio_interface.on_initalisation(&mut context);
        context.register_thread(NodeThread::RoutingThread);

        self.set_routing_delay(&mut context);
    }

    fn receive_message(
        &mut self,
        mut context: Context,
        header: &Self::InnerHeader,
        message_content: simulation::MessageContent,
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

        if header.dest.is_to_node(context.node_id()) {
            let ack_packet = StoredPacket {
                header: BasicHeader {
                    dest: Destination::Broadcast,
                    sender: context.node_id(),
                    packet_id: self.next_packet_id(),
                },
                message_content: MessageContent::NodeMessage(CustomContent::GlobalAck { id: key }),
                size: 0,
                snr: None,
            };

            self.acknowledge(&mut context, ack_packet.global_id());
            self.radio_interface.send(&mut context, ack_packet);
            self.acknowledge(&mut context, key);
            return;
        }

        if self.acknowledgements.contains(&key) {
            return;
        }

        match &packet.message_content {
            simulation::MessageContent::NodeMessage(custom_content) => match custom_content {
                super::CustomContent::GlobalAck { id } => {
                    self.remove_from_rebroadcasts(&mut context, *id);
                    self.acknowledge(&mut context, *id);
                }
                _ => (),
            },
            _ => (),
        }

        let was_removed = self.remove_from_rebroadcasts(&mut context, key);

        if was_removed {
            self.acknowledge(&mut context, key);
        } else {
            self.add_to_rebroadcasts(&mut context, packet);
        }
    }

    fn generate_message(
        &mut self,
        mut context: Context,
        message_id: simulation::MessageContent,
        message_info: &simulation::data_structs::MessageInfo,
    ) {
        let header = basic_header(context.node_id(), self.next_packet_id(), message_info);

        let packet = StoredPacket {
            header: header,
            message_content: message_id,
            size: message_info.size,
            snr: None,
        };

        self.add_to_rebroadcasts(&mut context, packet);
    }

    fn handle_error(&mut self, mut context: Context, error: simulation::NodeError) {
        match error {
            NodeError::RadioBusyError(_header, _content) => {
                context.log(|| "Radio busy error".into(), LogLevel::Error)
            }
        }
    }

    fn get_notified(
        &mut self,
        mut context: Context,
        notification: super::Notification,
        thread: super::NodeThread,
    ) {
        self.radio_interface
            .on_get_notified(&mut context, notification, thread);

        match notification {
            Notification::Routing => {
                self.run_routing_thread(&mut context);
            }
            _ => (),
        }
    }
}

impl Default for AcknowledgedOrRepeatFlood {
    fn default() -> Self {
        Self::new()
    }
}

impl AcknowledgedOrRepeatFlood {
    pub fn new() -> AcknowledgedOrRepeatFlood {
        AcknowledgedOrRepeatFlood {
            rebroadcasts: VecDeque::new(),
            acknowledgements: HashSet::new(),
            radio_interface: MeshtasticRadioInterface::new(),
            next_packet_id: 0,
        }
    }

    fn next_packet_id(&mut self) -> u32 {
        let out = self.next_packet_id;
        self.next_packet_id += 1;
        out
    }

    fn run_routing_thread(&mut self, context: &mut Context) {
        if let Some((packet, count)) = self.rebroadcasts.pop_front() {
            self.handle_dequeued_packet(context, packet, count);
        }

        self.set_routing_delay(context);
    }

    fn handle_dequeued_packet(
        &mut self,
        context: &mut Context<'_>,
        packet: StoredPacket<BasicHeader>,
        count: i32,
    ) {
        self.radio_interface.send(context, packet.clone());

        match packet.message_content {
            simulation::MessageContent::GeneratedMessage(_) => {
                if count > 0 {
                    self.rebroadcasts.push_back((packet, count - 1));
                } else {
                    self.acknowledge(context, packet.global_id());
                }
            }
            simulation::MessageContent::NodeMessage(_) => {
                self.acknowledge(context, packet.global_id());
            }
            _ => (),
        }
    }

    /// Add key to `self.acknowledgements` and do logging
    fn acknowledge(&mut self, context: &mut Context, key: GlobalPacketId) {
        self.acknowledgements.insert(key);
        context.log(
            || format!("{key:?} added to acknowledgements"),
            LogLevel::Debug,
        );
    }

    /// Returns true if the packet was in rebroadcasts and thus got removed.
    /// Returns false otherwise
    fn remove_from_rebroadcasts(&mut self, context: &mut Context, key: GlobalPacketId) -> bool {
        let rebroadcast_index = self
            .rebroadcasts
            .iter()
            .enumerate()
            .find_map(|(i, packet)| {
                if packet.0.global_id() == key {
                    Some(i)
                } else {
                    None
                }
            });

        let was_removed = if let Some(index) = rebroadcast_index {
            self.rebroadcasts.remove(index);
            self.radio_interface.cancel_sending(context, key);

            context.log(
                || format!("{key:?} was removed from rebroadcasts and sending cancelled"),
                LogLevel::Debug,
            );

            true
        } else {
            false
        };
        was_removed
    }

    fn set_routing_delay(&self, context: &mut Context<'_>) {
        let delay = Time::from_seconds(context.rng(1.0, 20.0));
        context.notify_later(
            delay,
            Notification::Routing,
            NodeThread::RoutingThread,
            true,
        );
    }

    fn add_to_rebroadcasts(&mut self, context: &mut Context, packet: StoredPacket<BasicHeader>) {
        let id = packet.global_id();
        context.log(
            || format!("{id:?} added to rebroadcast queue"),
            LogLevel::Debug,
        );
        self.rebroadcasts.push_front((packet, MAX_REBROADCASTS));
    }
}
