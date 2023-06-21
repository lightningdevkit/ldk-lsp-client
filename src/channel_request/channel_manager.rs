use std::collections::HashMap;
use std::convert::TryInto;
use std::ops::Deref;
use std::sync::{Arc, Mutex, RwLock};

use bitcoin::secp256k1::PublicKey;
use lightning::ln::msgs::{
	ChannelMessageHandler, ErrorAction, LightningError, OnionMessageHandler, RoutingMessageHandler,
};
use lightning::ln::peer_handler::{
	APeerManager, CustomMessageHandler, PeerManager, SocketDescriptor,
};
use lightning::routing::gossip::NetworkGraph;
use lightning::sign::{EntropySource, NodeSigner};
use lightning::util::logger::{Level, Logger};

use crate::transport::msgs::LSPSMessage;

const SUPPORTED_SPEC_VERSION: u16 = 1;

struct LiquidityChannel {
    id: u128,
	user_id: u128,
	state: ChannelState,
	api_version: u16,
	lsp_balance_sat: u64,
	client_balance_sat: u64,
	confirms_within_blocks: u32,
	channel_expiry_blocks: u32,
	token: String,
	created_at: String,
	expires_at: String,
	counterparty_node_id: PublicKey,
	scid: Option<u64>,
}

enum ChannelState{
    InfoRequested,
    PendingOrder,
    OrderRequested,
    PendingPayment,
    Ready,
}


struct ChannelRequest {
    
}

pub struct LiquidityManager< 	
    ES: Deref,
	Descriptor: SocketDescriptor + Send + Sync + 'static,
	L: Deref + Send + Sync + 'static,
	RM: Deref + Send + Sync + 'static,
	CM: Deref + Send + Sync + 'static,
	OM: Deref + Send + Sync + 'static,
	CMH: Deref + Send + Sync + 'static,
	NS: Deref + Send + Sync + 'static,
> where
	ES::Target: EntropySource,
	L::Target: Logger,
	RM::Target: RoutingMessageHandler,
	CM::Target: ChannelMessageHandler,
	OM::Target: OnionMessageHandler,
	CMH::Target: CustomMessageHandler,
	NS::Target: NodeSigner,
{
	entropy_source: ES,
	peer_manager: Mutex<Option<Arc<PeerManager<Descriptor, CM, RM, OM, L, CMH, NS>>>>,
	pending_messages: Arc<Mutex<Vec<(PublicKey, LSPSMessage)>>>,
	pending_events: Arc<Mutex<Vec<Event>>>,
	//per_peer_state: RwLock<HashMap<PublicKey, Mutex<PeerState>>>,
	channels_by_scid: RwLock<HashMap<u64, LiquidityChannel>>,
}


impl<
		ES: Deref,
		Descriptor: SocketDescriptor + Send + Sync + 'static,
		L: Deref + Send + Sync + 'static,
		RM: Deref + Send + Sync + 'static,
		CM: Deref + Send + Sync + 'static,
		OM: Deref + Send + Sync + 'static,
		CMH: Deref + Send + Sync + 'static,
		NS: Deref + Send + Sync + 'static,
	> LiquidityManager <ES, Descriptor, L, RM, CM, OM, CMH, NS>
where
	ES::Target: EntropySource,
	L::Target: Logger,
	RM::Target: RoutingMessageHandler,
	CM::Target: ChannelMessageHandler,
	OM::Target: OnionMessageHandler,
	CMH::Target: CustomMessageHandler,
	NS::Target: NodeSigner,
 {

    fn handle_get_info_request() {}

    fn handle_get_info_response() {}

    fn liquidity_source_selected() {}

    fn generate_order() {}

    fn handle_create_order_request() {}

    fn handle_create_order_response() {}

    fn handle_create_order_error() {}

    fn handle_get_order_request() {}

    fn handle_get_order_response() {}

    fn payment_for_channel() {}

    fn onchain_payment() {}

    fn lighnting_payment() {}

    fn check_payment_status() {}
    
    fn update_payment_status() {}

    fn channel_ready() {}

    fn update_order_status() {}

    fn channel_error() {}
    
}