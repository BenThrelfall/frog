use std::collections::{HashMap, HashSet, VecDeque};

use crate::{
    calculate_air_time,
    node::{
        BasicHeaderInfo, MeshPendingPacket, MeshStoredPacket, MeshtasticHeader,
    },
    simulation::{
        data_structs::{LogLevel, MessageInfo},
        Context, MessageContent, NodeError,
    },
    units::*,
    SNR_MAX, SNR_MIN,
};

use super::{
    CustomContent, Destination, GlobalPacketId, Header, ImplNodeModel, NodeThread, Notification,
    PendingPacket, RoutingStatus, StoredPacket,
};
pub(super) const DEFAULT_HOP_LIMIT: i32 = 3;

pub(super) const CW_MIN: i32 = 2;
pub(super) const CW_MAX: i32 = 7;
pub(super) const CW_DIFF: i32 = CW_MAX - CW_MIN;

pub(super) const SNR_DIFF: f64 = SNR_MAX - SNR_MIN;

// Consts from ReliableRouter.cpp
pub(super) const NUM_RETRANSMISSIONS: i32 = 3;

// TODO: We do not consider this apparent huge processing time and I guess we should do
// NOTE: I think the time is exagerated here but more real life testing is needed.
// Consts from RadioInterface
pub(super) const PROCESSING_TIME: Time = Time::from_milis(4500.0);

fn slot_time(bandwidth: Frequency, sf: i32) -> Time {
    let adjustment = Time::from_milis(0.2 + 0.4 + 7.0);
    let val = 8.5 * 2f64.powi(sf) / bandwidth + adjustment;
    val
}

/// Node model representing the default meshtastic protocol.
/// Uses the `MeshtasticRadioInterface` component and directly implements higher level routing logic.
/// It is currently largely unvalidated although simple inspection of simulation output using
/// this model appears correct up to intentional simplifications.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meshtastic {
    radio_interface: MeshtasticRadioInterface<MeshtasticHeader>,
    from_radio_queue: VecDeque<MeshStoredPacket>,
    pending: HashMap<GlobalPacketId, MeshPendingPacket>,
    seen_recently: HashSet<GlobalPacketId>,
    next_packet_id: u32,
}

use serde::{Deserialize, Serialize};
use Destination::*;
use Notification::*;

impl ImplNodeModel for Meshtastic {
    type InnerHeader = MeshtasticHeader;

    fn identity_str(&self) -> &str {
        "Meshtastic 1.2"
    }

    fn initalisation(&mut self, mut context: Context) {
        self.radio_interface.on_initalisation(&mut context);
        context.register_thread(NodeThread::RoutingThread);
    }

    fn receive_message(
        &mut self,
        mut context: Context,
        header: &MeshtasticHeader,
        message_content: MessageContent,
        payload_size: i32,
        snr: Db<f64>,
    ) {
        let packet = StoredPacket {
            header: header.clone(),
            message_content,
            size: payload_size,
            snr: Some(snr),
        };

        self.from_radio_queue.push_back(packet);
        context.notify_later(
            Time::from_milis(1.0),
            Routing,
            NodeThread::RoutingThread,
            true,
        );
    }

    fn generate_message(
        &mut self,
        mut context: Context,
        message_id: MessageContent,
        message_info: &MessageInfo,
    ) {
        let dest = message_info.std_destination();
        let header = MeshtasticHeader {
            dest,
            sender: context.node_id(),
            packet_id: self.next_packet_id(),
            hop_limit: DEFAULT_HOP_LIMIT,
            hop_start: DEFAULT_HOP_LIMIT,
            want_ack: true,
        };

        let packet = StoredPacket {
            header: header,
            message_content: message_id,
            size: message_info.size,
            snr: None,
        };

        self.send_local(&mut context, packet);

        context.log(
            || "send_local called for generated message".into(),
            LogLevel::Info,
        );
    }

    /// NOTE: Consider having a seperate function for each kind of error rathing than this error handling method
    fn handle_error(&mut self, mut context: Context, error: NodeError) {
        match error {
            NodeError::RadioBusyError(_header, _content) => {
                context.log(|| "Radio busy error".into(), LogLevel::Error)
            }
        }
    }

    /// Currently based on RadioLibInterface::onNotify
    fn get_notified(
        &mut self,
        mut context: Context,
        notification: Notification,
        _thread: NodeThread,
    ) {
        self.radio_interface
            .on_get_notified(&mut context, notification, _thread);

        match notification {
            Routing => {
                self.run_routing_thread(&mut context);
            }
            _ => (),
        }
    }
}

impl Default for Meshtastic {
    fn default() -> Self {
        Self::new()
    }
}

impl Meshtastic {
    pub fn new() -> Self {
        Self {
            radio_interface: MeshtasticRadioInterface::new(),
            from_radio_queue: VecDeque::new(),
            pending: HashMap::new(),
            seen_recently: HashSet::new(),
            next_packet_id: 0,
        }
    }

    // Private Methods

    fn run_routing_thread(&mut self, context: &mut Context) {
        //I think we'll pretend like this is coming out of the from radio queue
        //That does possibly mean that doRetransmissions is happening too often

        // STAGE ONE STUFF
        let delay = self.do_retransmissions(context);

        // STAGE TWO
        while let Some(packet) = self.from_radio_queue.pop_front() {
            self.perhaps_handle_received(context, packet);
        }

        // delay check is not part of the firmware
        // but it's clear that long delay is basically being used in place of
        // not having another notification.
        // This could probably be a shorter time but mainly we don't want the i32::MAX
        // length notifications.
        if delay < 48.0 * HOURS {
            // This is part of the firmware though of course
            context.notify_later(delay, Routing, NodeThread::RoutingThread, true);
        }
    }

    fn should_filter_received(&mut self, context: &mut Context, packet: &MeshStoredPacket) -> bool {
        // Reliable Router

        let key = GlobalPacketId {
            node_id: packet.header.sender,
            packet_id: packet.header.packet_id,
        };

        if packet.header.sender == context.node_id() {
            // If the node sees anyway rebroadcasting its packet it will stop retransmitting
            let old = self.pending.get(&key);

            if let Some(_) = old {
                self.send_ack_nak(
                    context,
                    RoutingStatus::NotError,
                    Node(packet.header.sender),
                    packet.header.packet_id,
                    0,
                );
                self.stop_retransmission(context, key);
            }
        }

        self.pending.iter_mut().for_each(|x| {
            x.1.next_tx = x.1.next_tx + calculate_air_time(packet.size, context.node_setting())
        });

        // Flooding Router
        if !self.was_seen_recently(context, packet) {
            // If not seen then don't filter
            return false;
        }

        // If it was seen then cancel and filter
        self.radio_interface.cancel_sending(context, key);

        let is_repeated =
            packet.header.hop_start > 0 && packet.header.hop_start == packet.header.hop_limit;

        if is_repeated {
            if !self.perhaps_rebroadcast(context, packet)
                && packet.header.dest.is_to_node(context.node_id())
                && packet.header.want_ack
            {
                self.send_ack_nak(
                    context,
                    RoutingStatus::NotError,
                    Node(packet.header.sender),
                    packet.header.packet_id,
                    0,
                );
            }
        }

        return true;
    }

    fn was_seen_recently(&mut self, context: &mut Context, packet: &MeshStoredPacket) -> bool {
        // We're assuming it's always withUpdate = true
        // Also we're ignoring expiry time

        let key = GlobalPacketId {
            node_id: packet.header.sender,
            packet_id: packet.header.packet_id,
        };

        let was_seen = self.seen_recently.contains(&key);

        if !was_seen {
            self.seen_recently.insert(key);
            context.log(
                || format!("{:?} added to seen_recently", key),
                LogLevel::Debug,
            );
        }

        was_seen
    }

    fn perhaps_rebroadcast(&mut self, context: &mut Context, packet: &MeshStoredPacket) -> bool {
        let to_us = packet.header.dest.is_to_node(context.node_id());
        let from_us = packet.header.sender == context.node_id();

        if !to_us && !from_us && packet.header.hop_limit > 0 {
            // We ignore the other conditions
            // They are not implemented in the sim

            let mut send_packet = packet.clone();

            send_packet.header.hop_limit -= 1;
            self.base_send(context, send_packet);

            return true;
        }

        return false;
    }

    fn start_retransmission(&mut self, context: &mut Context, packet: MeshStoredPacket) {
        let id = GlobalPacketId {
            node_id: packet.header.sender,
            packet_id: packet.header.packet_id,
        };

        let mut as_pending = PendingPacket {
            packet,
            next_tx: Time::from_seconds(0.0),
            num_retransmissions: NUM_RETRANSMISSIONS - 1,
        };

        self.stop_retransmission(context, id);
        Self::set_next_tx_for_pending(context, &mut as_pending);

        self.pending.insert(id, as_pending);
    }

    fn stop_retransmission(&mut self, context: &mut Context, key: GlobalPacketId) -> bool {
        if let Some(pend_packet) = self.pending.get(&key) {
            if pend_packet.num_retransmissions < NUM_RETRANSMISSIONS - 1 {
                self.radio_interface.cancel_sending(context, key);
            }

            let res = self.pending.remove(&key);
            assert!(res.is_some());
            true
        } else {
            false
        }
    }

    fn do_retransmissions(&mut self, context: &mut Context) -> Time {
        let now = context.clock_time();
        let mut delay = Time::from_imilis(i32::MAX);

        let mut stop_keys = Vec::new();
        let mut send_packets = Vec::new();

        for (key, packet) in self.pending.iter_mut() {
            let mut still_valid = true;

            if packet.next_tx <= now {
                if packet.num_retransmissions == 0 {
                    stop_keys.push(*key);
                    still_valid = false;
                } else {
                    send_packets.push(packet.packet.clone());
                    packet.num_retransmissions -= 1;
                    Self::set_next_tx_for_pending(context, packet);
                }
            }

            if still_valid {
                delay = delay.min(packet.next_tx - now);
            }
        }

        for packet in send_packets {
            self.flood_send(context, packet);
        }

        for key in stop_keys {
            let packet = self.pending.get(&key).expect("key must still be valid");

            self.send_ack_nak(
                context,
                RoutingStatus::MaxRetransmit,
                Node(packet.packet.header.sender),
                packet.packet.header.packet_id,
                0,
            );

            self.stop_retransmission(context, key);
        }

        return delay;
    }

    fn set_next_tx_for_pending(context: &mut Context, packet: &mut MeshPendingPacket) {
        let delay = Self::get_retransmission_delay(context, &packet.packet);
        packet.next_tx = context.clock_time() + delay;

        // In most cases this gets overriden by the do_retransmissions delay
        // NOTE: Maybe theres a better way to do this
        context.notify_later(
            Time::from_milis(1.0),
            Routing,
            NodeThread::RoutingThread,
            true,
        );
    }

    fn send_ack_nak(
        &mut self,
        context: &mut Context,
        status: RoutingStatus,
        dest: Destination,
        packet_id: u32,
        hop_limit: i32,
    ) {
        let packet = StoredPacket {
            header: MeshtasticHeader {
                dest,
                sender: context.node_id(),
                packet_id: self.next_packet_id(),
                hop_limit: hop_limit,
                hop_start: hop_limit,
                want_ack: false,
            },
            message_content: MessageContent::NodeMessage(CustomContent::RoutingMessage {
                status,
                about_id: packet_id,
            }),
            size: 0,
            snr: None,
        };

        self.send_local(context, packet);
    }

    fn send_local(&mut self, context: &mut Context, packet: MeshStoredPacket) {
        if packet.header.dest.is_to_node(context.node_id()) {
            // should be equiv to enqueueReceivedMessage call
            self.from_radio_queue.push_back(packet.clone());
            context.notify_later(
                Time::from_milis(1.0),
                Routing,
                NodeThread::RoutingThread,
                true,
            );
        } else {
            if packet.header.dest.is_broadcast() {
                self.handle_received(context, &packet);
            }

            self.reliable_send(context, packet);
        }
    }

    fn reliable_send(&mut self, context: &mut Context, mut packet: MeshStoredPacket) {
        if packet.header.want_ack {
            if packet.header.hop_limit == 0 {
                packet.header.hop_limit = DEFAULT_HOP_LIMIT;
            }

            self.start_retransmission(context, packet.clone());
        }

        self.pending
            .iter_mut()
            .filter(|x| x.0.packet_id != packet.header.packet_id)
            .for_each(|x| {
                x.1.next_tx = x.1.next_tx + calculate_air_time(packet.size, context.node_setting())
            });

        self.flood_send(context, packet);
    }

    fn flood_send(&mut self, context: &mut Context, packet: MeshStoredPacket) {
        self.was_seen_recently(context, &packet);
        self.base_send(context, packet);
    }

    fn base_send(&mut self, context: &mut Context, mut packet: MeshStoredPacket) {
        if packet.header.dest.is_to_node(context.node_id()) {
            panic!("This shouldn't happen. Though maybe should panic either");
        }

        if self.violating_duty_cycle(context) {
            // TODO: Implement correct behaviour
            context.log(|| ("Violating Duty Cycle").to_string(), LogLevel::Error);
        }

        if packet.header.dest.is_broadcast() {
            packet.header.want_ack = false;
        }

        if packet.header.sender == context.node_id() {
            packet.header.hop_start = packet.header.hop_limit;
        }

        // We're not simulating encryption rn (and probably won't)

        self.radio_interface.send(context, packet);
    }

    fn violating_duty_cycle(&mut self, _context: &mut Context) -> bool {
        // TODO
        false
    }

    fn perhaps_handle_received(&mut self, context: &mut Context, packet: MeshStoredPacket) {
        if self.should_filter_received(context, &packet) {
            return;
        }

        self.handle_received(context, &packet);
    }

    fn handle_received(&mut self, context: &mut Context, packet: &MeshStoredPacket) {
        context.log(|| format!("Received {:?}", packet), LogLevel::Info);

        //I'm pretty sure the only module that needs taking into account is the
        // routing module but I could be wrong

        //On that assumption just call directly into routing module
        self.routing_module_handle_received(context, packet);
    }

    fn routing_module_handle_received(&mut self, context: &mut Context, packet: &MeshStoredPacket) {
        // Ignoring foreign mesh stuff. Assume everything is from a single mesh
        // maybe eventually account for forign meshes but idk

        self.reliable_sniff_received(context, packet);

        // Can probably ignore handleFromRadio as well
    }

    fn get_hop_limit_for_response(hop_start: i32, hop_limit: i32) -> i32 {
        if hop_start != 0 {
            let hops_used = if hop_start < hop_limit {
                DEFAULT_HOP_LIMIT
            } else {
                hop_start - hop_limit
            };

            if hops_used > DEFAULT_HOP_LIMIT {
                // Assuming not event mode
                return hops_used;
            } else if hops_used + 2 < DEFAULT_HOP_LIMIT {
                return hops_used + 2;
            }
        }

        return DEFAULT_HOP_LIMIT;
    }

    fn reliable_sniff_received(&mut self, context: &mut Context, packet: &MeshStoredPacket) {
        let is_to_us = packet.header.dest.is_to_node(context.node_id());

        let routing_content = match &packet.message_content {
            MessageContent::NodeMessage(custom_content) => match custom_content {
                CustomContent::RoutingMessage { status, about_id } => Some((status, about_id)),
                _ => None,
            },
            _ => None,
        };

        if is_to_us {
            if packet.header.want_ack {
                // Assuming everything is sorta decrypted and in channel simplifies things

                // I think checking decoded.request_id is the same as the check below (in effect)
                // because a request_id will only exist if itwas a routing packet
                // could be wrong though

                if routing_content.is_none() {
                    self.send_ack_nak(
                        context,
                        RoutingStatus::NotError,
                        Node(packet.header.sender),
                        packet.header.packet_id,
                        Self::get_hop_limit_for_response(
                            packet.header.hop_start,
                            packet.header.hop_limit,
                        ),
                    );
                } else if packet.header.hop_start > 0
                    && packet.header.hop_start == packet.header.hop_limit
                {
                    self.send_ack_nak(
                        context,
                        RoutingStatus::NotError,
                        Node(packet.header.sender),
                        packet.header.packet_id,
                        0,
                    );
                }
            }

            // Not certain this is entirely correct
            // in firmware decoded.request_id field is checked even if meshtastic_Routing
            // is null but see above

            if let Some((status, id)) = routing_content {
                match status {
                    // Yes both branches are exactly the same
                    // might be useful for logging later
                    RoutingStatus::NotError => {
                        // This is an Ack
                        self.stop_retransmission(
                            context,
                            GlobalPacketId {
                                node_id: context.node_id(),
                                packet_id: *id,
                            },
                        );
                    }
                    _ => {
                        // This is a Nak
                        self.stop_retransmission(
                            context,
                            GlobalPacketId {
                                node_id: context.node_id(),
                                packet_id: *id,
                            },
                        );
                    }
                }
            }
        }

        // Same logic as above for telling if its ack or reply
        self.flood_sniff_received(context, packet, routing_content.is_some());
    }

    fn flood_sniff_received(
        &mut self,
        context: &mut Context,
        packet: &MeshStoredPacket,
        was_ack_or_reply: bool,
    ) {
        let is_to_us = packet.header.dest.is_to_node(context.node_id());

        if was_ack_or_reply && !is_to_us && !packet.header.dest.is_broadcast() {
            self.radio_interface
                .cancel_sending(context, packet.global_id());
        }

        self.perhaps_rebroadcast(context, packet);

        self.base_sniff_received(context, packet);
    }

    fn base_sniff_received(&mut self, _context: &mut Context, _packet: &MeshStoredPacket) {
        // Doesn't do anything
    }

    fn next_packet_id(&mut self) -> u32 {
        let out = self.next_packet_id;
        self.next_packet_id += 1;
        out
    }

    fn get_retransmission_delay(context: &mut Context, packet: &MeshStoredPacket) -> Time {
        let airtime = calculate_air_time(packet.size, context.node_setting());
        let window_size = (context.channel_utilisation() * CW_DIFF as f64).floor() as i32 + CW_MIN;

        let settings = context.node_setting();
        2.0 * airtime
            + Time::from_milis(2f64.powi(window_size) + 2.0 * CW_MAX as f64)
            + 2f64.powi(CW_MAX + CW_MIN / 2) * slot_time(settings.bandwidth, settings.sf)
            + PROCESSING_TIME
    }
}

// Components

/// This component uses the logic from Meshtastic but does not require the Meshtastic header.
/// - `T` - Header type ([`super::MeshtasticHeader`] or [`super::BasicHeader`] or custom header)
///
/// Component representing the default meshtastic radio interface (RadioInterface.cpp and RadioLibInterface.cpp)
/// Most methods on this struct mirror methods found in the above cpp classes.
/// It performs collision avoidance using channel access detection and randomised slot based delays.
///
/// To use this component:
/// - Add the `MeshtasticRadioInterface::on_get_notified` method in `get_notified`
/// - Add the `MeshtasticRadioInterface::on_initalisation` method to `initalisation`
///
/// Use `MeshtasticRadioInterface::send` to queue messages to be broadcast.
/// To cancel the broadcast of a queued packet call `MeshtasticRadioInterface::cancel_sending`.
///
/// This component uses the [`NodeThread::RadioThread`]. For normal behavour do not use this elsewhere in your node model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshtasticRadioInterface<T> {
    tx_queue: VecDeque<StoredPacket<T>>,
}

impl<T> MeshtasticRadioInterface<T>
where
    T: BasicHeaderInfo + Into<Header>,
{
    // Hooks

    pub(super) fn on_initalisation(&self, context: &mut Context) {
        context.register_thread(NodeThread::RadioThread);
    }

    pub(super) fn on_get_notified(
        &mut self,
        context: &mut Context,
        notification: Notification,
        _thread: NodeThread,
    ) {
        match notification {
            TransmitDelayCompleted => {
                if self.tx_queue.is_empty() == false {
                    // Some chance channel_in_use is not correct here
                    // Possibly needs to be receiving a packet not just detecting use
                    if context.is_transmitting() || context.channel_in_use() {
                        self.set_transmit_delay(context);
                    } else {
                        let packet = self
                            .tx_queue
                            .pop_front()
                            .expect("already checked queue is not empty");

                        context.enqueue_send(packet.header, packet.message_content);

                        // Added because otherwise this thread won't get called again
                        // unless a new message is enqueued for send even if queue not empty
                        // This isn't in the meshtastic firmware though
                        self.set_transmit_delay(context);
                    }
                }
            }
            _ => (),
        }
    }

    // Other Functions

    pub fn new() -> Self {
        Self {
            tx_queue: VecDeque::new(),
        }
    }

    fn set_transmit_delay(&mut self, context: &mut Context) {
        let Some(packet) = self.tx_queue.front() else {
            return;
        };

        match packet.snr {
            Some(inner) => {
                let delay = Self::get_weighted_tx_delay(inner, context);
                context.notify_later(
                    delay,
                    TransmitDelayCompleted,
                    NodeThread::RadioThread,
                    false,
                );
            }
            None => {
                let delay = Self::get_tx_delay(context);
                context.notify_later(
                    delay,
                    TransmitDelayCompleted,
                    NodeThread::RadioThread,
                    false,
                );
            }
        }
    }

    fn get_weighted_tx_delay(snr: Db<f64>, context: &mut Context) -> Time {
        let use_snr = snr.as_db_float();

        let unity_snr = ((use_snr - SNR_MIN) / SNR_DIFF).clamp(0.0, 1.0);
        let window_size = (unity_snr * CW_DIFF as f64).floor() as i32 + CW_MIN;
        let multiplier = 2.0 * CW_MAX as f64 + context.rng(0.0, 2f64.powi(window_size)).floor();

        let settings = context.node_setting();
        multiplier * slot_time(settings.bandwidth, settings.sf)
    }

    fn get_tx_delay(context: &mut Context) -> Time {
        let window_size = (context.channel_utilisation() * CW_DIFF as f64).floor() as i32 + CW_MIN;
        let multiplier = context.rng(0.0, 2f64.powi(window_size)).floor();

        let settings = context.node_setting();
        multiplier * slot_time(settings.bandwidth, settings.sf)
    }

    pub(super) fn send(&mut self, context: &mut Context, packet: StoredPacket<T>) {
        self.tx_queue.push_back(packet);
        self.set_transmit_delay(context);
    }

    /// Non-meshtastic. Add packet to the front of the queue not the back.
    pub(super) fn priority_send(&mut self, context: &mut Context, packet: StoredPacket<T>) {
        self.tx_queue.push_front(packet);
        self.set_transmit_delay(context);
    }

    pub(super) fn cancel_sending(&mut self, context: &mut Context, key: GlobalPacketId) -> bool {
        context.log(
            || format!("cancel_sending called for {:?}", key),
            LogLevel::Debug,
        );
        let maybe_index = self.tx_queue.iter().enumerate().find_map(|(i, x)| {
            if x.header.sender() == key.node_id && x.header.packet_id() == key.packet_id {
                Some(i)
            } else {
                None
            }
        });

        if let Some(index) = maybe_index {
            self.tx_queue.remove(index);
            true
        } else {
            false
        }
    }
}
