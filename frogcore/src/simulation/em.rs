use super::*;
use crate::{calculate_air_time, context};

impl Simulation {
    /// Returns a new ID for a new transmission struct.
    pub(super) fn new_trans_id(&mut self) -> u32 {
        let tmp = self.next_trans_id;
        self.next_trans_id += 1;
        tmp
    }

    /// Returns true if the node is transmitting false otherwise.
    pub(super) fn is_transmitting(&self, node_id: usize) -> bool {
        self.active_transmissions()
            .find(|x| x.transmitter_id == node_id)
            .is_some()
    }

    pub(super) fn active_transmissions(&self) -> impl Iterator<Item = &Transmission> {
        self.em_field
            .iter()
            .rev()
            .take_while(|x| x.end_time >= self.sim_time)
    }

    pub(super) fn message_size(&self, message_content: &MessageContent) -> i32 {
        match message_content {
            MessageContent::GeneratedMessage(id) => self.test_messages[*id].size,
            MessageContent::NodeMessage(custom_content) => custom_content.size(),
            MessageContent::Empty => 0,
        }
    }

    /// Try to broadcast something. This will send an error to the transmitter node if a broadcast cannot be started.
    ///
    /// * `message_size` - Size of the packet body in bytes.
    pub(super) fn try_broadcast(
        &mut self,
        sender_id: usize,
        header: Header,
        message_content: MessageContent,
    ) {
        if self.is_transmitting(sender_id) {
            let context = context!(self, sender_id);
            self.nodes[sender_id]
                .handle_error(context, NodeError::RadioBusyError(header, message_content));

            return;
        }

        let transmission_id = self.new_trans_id();

        let settings = &self.node_settings[sender_id];
        let message_size = self.message_size(&message_content);
        let end_time = self.sim_time + calculate_air_time(message_size + header.size(), settings);

        let transmission = Transmission {
            id: transmission_id,
            start_time: self.sim_time,
            end_time: end_time,
            sf: settings.sf,
            power: settings.use_power,
            bandwidth: settings.bandwidth,
            carrier_band: settings.carrier_band,
            transmitter_id: sender_id,
            header: header,
            message_content,
        };

        self.insert_transmission(transmission);

        // Loop over adj nodes and add recieve event
        for id in self.graph.get_adj(sender_id) {
            self.event_queue.push(SimEvent {
                time: end_time,
                action: SimAction::RecieveMessage {
                    node_id: id,
                    transmission_id,
                },
            });
        }

        self.logs.push(LogItem {
            time: self.sim_time,
            log_level: LogLevel::Info,
            source: LogSource::Simulation,
            content: LogContent::TransmissionSent {
                sender_id,
                transmission_id,
            },
        });
    }

    /// Insert transmission into em_field based on its end_time
    pub(super) fn insert_transmission(&mut self, transmission: Transmission) {
        let insert_pos = self
            .em_field
            .iter()
            .enumerate()
            .rev()
            .find(|(_, x)| x.end_time < transmission.end_time)
            .map_or(0, |(n, _)| n + 1);

        self.em_field.insert(insert_pos, transmission);
    }
}
