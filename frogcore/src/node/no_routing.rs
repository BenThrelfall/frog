use crate::{node::{basic_header, BasicHeader}, simulation::{data_structs::LogLevel, NodeError}};

use super::{ImplNodeModel};

use serde::{Deserialize, Serialize};
/// Sends generated messages ASAP but otherwise does nothing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoRouting {
    next_packet_id: u32,
}

impl ImplNodeModel for NoRouting {
    type InnerHeader = BasicHeader;
    fn identity_str(&self) -> &str {
        "No Routing 1.0"
    }

    fn initalisation(&mut self, _context: crate::simulation::Context) {}

    fn receive_message(
        &mut self,
        _context: crate::simulation::Context,
        _header: &Self::InnerHeader,
        _message_content: crate::simulation::MessageContent,
        _payload_size: i32,
        _snr: crate::units::Db<f64>,
    ) {
    }

    fn generate_message(
        &mut self,
        mut context: crate::simulation::Context,
        message_id: crate::simulation::MessageContent,
        message_info: &crate::simulation::data_structs::MessageInfo,
    ) {
        // Although this is no routing. Using a packet id can make debuging easier.

        let header = basic_header(context.node_id(), self.next_packet_id, &message_info);
        self.next_packet_id += 1;

        context.enqueue_send(header, message_id);
    }

    fn handle_error(
        &mut self,
        mut context: crate::simulation::Context,
        error: crate::simulation::NodeError,
    ) {
        match error {
            // One of the only node models where this will happen.
            // Gets a more detailed message.
            NodeError::RadioBusyError(header, message_content) => {
                context.log(||format!("Radio Busy! The following packet was dropped:\n{header:#?}\n{message_content:?}"), LogLevel::Error)
            }
        }
    }

    fn get_notified(
        &mut self,
        _context: crate::simulation::Context,
        _notification: super::Notification,
        _thread: super::NodeThread,
    ) {
    }
}
impl Default for NoRouting {
    fn default() -> Self {
        Self::new()
    }
}
impl NoRouting {
    pub fn new() -> NoRouting {
        NoRouting { next_packet_id: 0 }
    }
}
