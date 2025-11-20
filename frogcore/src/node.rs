pub mod ack_repeat_flood;
pub mod basic_flood;
pub mod meshtastic;
pub mod no_routing;
pub mod probabilistic_flood;
pub mod simple_managed_flooding;
pub mod stack_flood;

use thiserror::Error;

use crate::{
    simulation::{data_structs::MessageInfo, Context, MessageContent, NodeError},
    units::{Db, Time},
};

pub use ack_repeat_flood::AcknowledgedOrRepeatFlood;
pub use basic_flood::BasicFlood;
pub use meshtastic::Meshtastic;
pub use no_routing::NoRouting;
pub use probabilistic_flood::ProbabilisticFlood;
pub use serde::{Deserialize, Serialize};
pub use simple_managed_flooding::SimpleManagedFlooding;
pub use stack_flood::StackFlood;

macro_rules! node_model {
    ($count:literal, $($variant:ident),+) => {

        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub enum NodeModel {
            $(
                $variant($variant),
            )*
        }

        impl ImplNodeModel for NodeModel {

            type InnerHeader = Header;

            fn identity_str(&self) -> &str {
                match self {
                    $(
                        NodeModel::$variant(inner) => inner.identity_str(),
                    )*
                }
            }

            fn initalisation(&mut self, context: Context) {
                match self {
                    $(
                        NodeModel::$variant(inner) => inner.initalisation(context),
                    )*
                }
            }

            fn receive_message(
                &mut self,
                context: Context,
                header: &Self::InnerHeader,
                message_content: MessageContent,
                payload_size: i32,
                snr: Db<f64>,
            ) {
                match self {


                    $(
                        NodeModel::$variant(inner) => {
                            let Ok(inner_header) = header.try_into() else {
                                return;
                            };

                            inner.receive_message(context, inner_header, message_content, payload_size, snr);
                        },
                    )*
                }
            }

            fn generate_message(
                &mut self,
                context: Context,
                message_id: MessageContent,
                message_info: &MessageInfo,
            ) {
                match self {
                    $(
                        NodeModel::$variant(inner) => inner.generate_message(context, message_id, message_info),
                    )*
                }
            }

            fn handle_error(&mut self, context: Context, error: NodeError) {
                match self {
                    $(
                        NodeModel::$variant(inner) => inner.handle_error(context, error),
                    )*
                }
            }

            fn get_notified(&mut self, context: Context, notification: Notification, thread: NodeThread) {
                match self {
                    $(
                        NodeModel::$variant(inner) => inner.get_notified(context, notification, thread),
                    )*
                }
            }
        }

        $(

        impl From<$variant> for NodeModel {
            fn from(value: $variant) -> Self {
                NodeModel::$variant(value)
            }
        }

        )*

        #[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
        pub enum ModelSelection {
            $(
                $variant
            ),*
        }

        impl From<ModelSelection> for NodeModel {
            fn from(value: ModelSelection) -> Self {
                match value {
                    $(
                        ModelSelection::$variant => $variant::default().into()
                    ),*
                }
            }
        }

        pub const MODEL_LIST : [ModelSelection; $count] = [
            $(
                ModelSelection::$variant
            ),*
        ];

    };
}

node_model!(
    7,
    Meshtastic,
    AcknowledgedOrRepeatFlood,
    BasicFlood,
    StackFlood,
    NoRouting,
    ProbabilisticFlood,
    SimpleManagedFlooding
);

#[derive(Debug, Error)]
#[error("Failed to parse string to node model")]
pub struct ParseModelError;

pub fn parse_model(s: &str) -> Result<ModelSelection, ParseModelError> {
    use ModelSelection::*;

    Ok(match s.to_lowercase().as_str() {
        "meshtastic" => Meshtastic,
        "big_flood" | "bigflood" | "ack_flood" | "repeat_flood" => AcknowledgedOrRepeatFlood,
        "flood" | "basic_flood" | "basicflood" => BasicFlood,
        "stackflood" | "stack flood" | "stack_flood" => StackFlood,
        "probabilisticflood" | "probabilistic_flood" => ProbabilisticFlood,
        "norouting" | "no_routing" => NoRouting,
        _ => return Err(ParseModelError),
    })
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum NodeThread {
    RadioThread,
    RoutingThread,
    CacheThread,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CustomContent {
    RoutingMessage {
        status: RoutingStatus,
        about_id: u32,
    },
    GlobalAck {
        id: GlobalPacketId,
    },
}

impl CustomContent {
    /// Returns the size in bytes
    pub fn size(&self) -> i32 {
        match self {
            CustomContent::RoutingMessage { .. } => 8,
            CustomContent::GlobalAck { .. } => 8,
        }
    }
}

/// Called meshtastic_Routing_Error in cpp.
/// Renamed as its not a simulation error
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum RoutingStatus {
    NotError,
    MaxRetransmit,
}

/// A representation of a simulated radio node. Implement this trait to create custom node models
/// for simulating custom routing methods.
///
/// [`Context`] is used throughout to give access the the underlying simulation. It should be used
/// for:
///
/// - transmitting messages
/// - changing radio settings
/// - generating random numbers
///
/// See the [`Context`] documentation for more details
pub trait ImplNodeModel {
    type InnerHeader;

    /// An identifier for the node model.
    /// Should depend only on any options the node model may have when being created.
    /// It should include a version number that is incremented when the node model is updated.
    fn identity_str(&self) -> &str;

    /// Called once at the start of the simulation.
    /// [`Context::register_thread`] should be called here for every simulated
    /// the node model uses.
    fn initalisation(&mut self, context: Context);

    /// Called from the simulation when the node finishes receiving a transmission successfully.
    fn receive_message(
        &mut self,
        context: Context,
        header: &Self::InnerHeader,
        message_content: MessageContent,
        payload_size: i32,
        snr: Db<f64>,
    );

    /// This function is called from the simulation when a simulated user generates a message to be sent by the node.
    ///
    /// - `message_id` - An id for the simulator to identify the generated message. Make sure it is transmitted (see [`Context::enqueue_send`]).
    /// Should always be a [`MessageContent::GeneratedMessage`].
    /// - `message_info` - Information about the simulated generated message.
    fn generate_message(
        &mut self,
        context: Context,
        message_id: MessageContent,
        message_info: &MessageInfo,
    );

    // NOTE: Consider having a seperate function for each kind of error rathing than this error handling method
    /// Handles errors that may be raised because of interaction between the node model and simulation.
    /// These are simulated node errors not errors in the simulator itself.
    ///
    /// Currently the only error is [`NodeError::RadioBusyError`] which occurs if the node model tries to transmit while already transmitting.
    fn handle_error(&mut self, context: Context, error: NodeError);

    /// Works like a meshtastic notified worker thread. Multiple simulated threads can be set up with [`Context::register_thread`].
    /// Then for each simulated thread notifications can be registered using [`Context::notify_later`].
    /// There can only be one notification per time per thread meaning trying to register a new notification for the same thread will
    /// either override the existing one or be ignored. See [`Context::notify_later`]
    fn get_notified(&mut self, context: Context, notification: Notification, thread: NodeThread);
}

#[derive(Clone, Copy, Debug)]
pub enum Notification {
    TransmitDelayCompleted,
    Routing,
    InfoTimer,
    CachedHost,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Header {
    Basic(BasicHeader),
    Meshtastic(MeshtasticHeader),
}

pub trait BasicHeaderInfo {
    fn dest(&self) -> Destination;
    fn sender(&self) -> usize;
    fn packet_id(&self) -> u32;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BasicHeader {
    dest: Destination,
    sender: usize,
    packet_id: u32,
}

impl BasicHeaderInfo for BasicHeader {
    fn dest(&self) -> Destination {
        self.dest
    }

    fn sender(&self) -> usize {
        self.sender
    }

    fn packet_id(&self) -> u32 {
        self.packet_id
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshtasticHeader {
    dest: Destination,
    sender: usize,
    packet_id: u32,
    hop_limit: i32,
    hop_start: i32,
    want_ack: bool,
}

impl BasicHeaderInfo for MeshtasticHeader {
    fn dest(&self) -> Destination {
        self.dest
    }

    fn sender(&self) -> usize {
        self.sender
    }

    fn packet_id(&self) -> u32 {
        self.packet_id
    }
}

impl Header {
    /// Implement a function to calculate the header size in bytes.
    /// Will be constant in many cases.
    pub fn size(&self) -> i32 {
        16 // default for Meshtastic
    }
}

impl TryFrom<Header> for BasicHeader {
    type Error = ();

    fn try_from(value: Header) -> Result<Self, Self::Error> {
        match value {
            Header::Basic(basic_header) => Ok(basic_header),
            _ => Err(()),
        }
    }
}

impl TryFrom<Header> for MeshtasticHeader {
    type Error = ();

    fn try_from(value: Header) -> Result<Self, Self::Error> {
        match value {
            Header::Meshtastic(meshtastic_header) => Ok(meshtastic_header),
            _ => Err(()),
        }
    }
}

impl<'a> TryFrom<&'a Header> for &'a BasicHeader {
    type Error = ();

    fn try_from(value: &'a Header) -> Result<Self, Self::Error> {
        match value {
            Header::Basic(basic_header) => Ok(basic_header),
            _ => Err(()),
        }
    }
}

impl<'a> TryFrom<&'a Header> for &'a MeshtasticHeader {
    type Error = ();

    fn try_from(value: &'a Header) -> Result<Self, Self::Error> {
        match value {
            Header::Meshtastic(meshtastic_header) => Ok(meshtastic_header),
            _ => Err(()),
        }
    }
}

impl From<BasicHeader> for Header {
    fn from(value: BasicHeader) -> Self {
        Header::Basic(value)
    }
}

impl From<MeshtasticHeader> for Header {
    fn from(value: MeshtasticHeader) -> Self {
        Header::Meshtastic(value)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Destination {
    Broadcast,
    Node(usize),
}

impl Destination {
    /// Returns false if broadcast. Otherwise returns true if the destination is the provided node id
    fn is_to_node(self, node_id: usize) -> bool {
        match self {
            Destination::Broadcast => false,
            Destination::Node(id) => id == node_id,
        }
    }

    fn is_broadcast(self) -> bool {
        match self {
            Destination::Broadcast => true,
            Destination::Node(_) => false,
        }
    }
}

// Structs that are generally useful for different node models

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlobalPacketId {
    node_id: usize,
    packet_id: u32,
}

pub type MeshStoredPacket = StoredPacket<MeshtasticHeader>;
pub type BasicStoredPacket = StoredPacket<BasicHeader>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredPacket<H> {
    header: H,
    message_content: MessageContent,
    size: i32,
    snr: Option<Db<f64>>,
}

impl<T> StoredPacket<T>
where
    T: BasicHeaderInfo,
{
    fn global_id(&self) -> GlobalPacketId {
        GlobalPacketId {
            node_id: self.header.sender(),
            packet_id: self.header.packet_id(),
        }
    }
}

type MeshPendingPacket = PendingPacket<MeshtasticHeader>;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingPacket<H> {
    packet: StoredPacket<H>,
    next_tx: Time,
    num_retransmissions: i32,
}

/// Function for creating a standard header for a user generated message.
fn basic_header(sender_id: usize, packet_id: u32, message_info: &MessageInfo) -> BasicHeader {
    let dest = if message_info.targets.len() == 1 {
        Destination::Node(*message_info.targets.first().expect("checked length"))
    } else {
        Destination::Broadcast
    };

    let header = BasicHeader {
        dest,
        sender: sender_id,
        packet_id: packet_id,
    };

    header
}

/// Function for creating a standard header for a user generated message.
fn meshtastic_header(
    sender_id: usize,
    packet_id: u32,
    message_info: &MessageInfo,
) -> MeshtasticHeader {
    let dest = if message_info.targets.len() == 1 {
        Destination::Node(*message_info.targets.first().expect("checked length"))
    } else {
        Destination::Broadcast
    };

    let header = MeshtasticHeader {
        hop_limit: 3,
        dest,
        sender: sender_id,
        packet_id: packet_id,
        hop_start: 3,
        want_ack: true,
    };

    header
}
