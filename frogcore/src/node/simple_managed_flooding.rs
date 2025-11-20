use std::collections::HashSet;

use crate::{
    node::{basic_header, BasicHeader},
    simulation::{data_structs::LogLevel, NodeError},
};

use super::{
    meshtastic::MeshtasticRadioInterface, GlobalPacketId, ImplNodeModel,
    StoredPacket,
};

use serde::{Deserialize, Serialize};
/// A version of managed flooding without rebroadcasting or acknowledgements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleManagedFlooding {
    seen: HashSet<GlobalPacketId>,
    sent: HashSet<GlobalPacketId>,
    radio_interface: MeshtasticRadioInterface<BasicHeader>,
    next_packet_id: u32,
}

impl ImplNodeModel for SimpleManagedFlooding {
    type InnerHeader = BasicHeader;
    fn identity_str(&self) -> &str {
        "Simple Managed Flooding 1.0"
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
            message_content: message_content,
            size: payload_size,
            snr: Some(snr),
        };

        let key = packet.global_id();

        // Regular managed flooding early return if we've seen the msg
        if self.seen.contains(&key) {
            self.radio_interface.cancel_sending(&mut context, key);
            return;
        }

        if !packet.header.dest.is_to_node(context.node_id()) && !self.sent.contains(&key) {
            context.log(
                || format!("Enqueuing rebroadcast for {key:?}"),
                LogLevel::Info,
            );
            self.sent.insert(key);
            self.radio_interface.send(&mut context, packet);
        }

        self.seen.insert(key);
    }

    fn generate_message(
        &mut self,
        mut context: crate::simulation::Context,
        message_id: crate::simulation::MessageContent,
        message_info: &crate::simulation::data_structs::MessageInfo,
    ) {
        let header = basic_header(context.node_id(), self.next_packet_id(), message_info);

        let packet = StoredPacket {
            header: header,
            // Acceptable to clone here because we know its not a custom content
            message_content: message_id.clone(),
            size: message_info.size,
            snr: None,
        };
        let key = packet.global_id();

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
        self.sent.insert(key);
        self.seen.insert(key);
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
impl Default for SimpleManagedFlooding {
    fn default() -> Self {
        Self::new()
    }
}
impl SimpleManagedFlooding {
    pub fn new() -> Self {
        SimpleManagedFlooding {
            seen: HashSet::new(),
            sent: HashSet::new(),
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
