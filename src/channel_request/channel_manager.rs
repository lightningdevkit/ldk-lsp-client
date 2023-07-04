use std::collections::HashMap;
use std::convert::TryInto;
use std::ops::Deref;
use std::sync::{Arc, Mutex, RwLock};

use bitcoin::hashes::Hash;
use bitcoin::secp256k1::PublicKey;
use lightning::ln::msgs::{
	ChannelMessageHandler, ErrorAction, LightningError, OnionMessageHandler, RoutingMessageHandler,
};
use lightning::ln::peer_handler::{
	APeerManager, CustomMessageHandler, PeerManager, SocketDescriptor,
};
use lightning::routing::gossip::NetworkGraph;
use lightning::sign::{EntropySource, NodeSigner};
use lightning::util::errors::APIError;
use lightning::util::logger::{Level, Logger};

use crate::channel_request::msgs::{CreateOrderRequest, Message, Order, Request};
use crate::transport::message_handler::ProtocolMessageHandler;
use crate::transport::msgs::{LSPSMessage, RequestId};
use crate::utils;
use crate::{events::Event, transport::msgs::ResponseError};

use super::event;
use super::msgs::{
	ChannelInfo, CreateOrderResponse, GetInfoRequest, GetInfoResponse, GetOrderRequest,
	GetOrderResponse, OnchainPayment, OptionsSupported, OrderId, OrderState, Payment, PaymentState,
	Response,
};
use super::utils::check_if_valid;

const SUPPORTED_SPEC_VERSION: u16 = 1;

#[derive(PartialEq)]
enum ChannelState {
	InfoRequested,
	OrderRequested,
	PendingSelection,
	PendingPayment,
	Ready,
}

struct CRchannel {
	channel_id: u128,
	user_id: u128,
	counterparty_node_id: PublicKey,
	state: ChannelState,
	//supported_version: Option<u8>,
	lsp_balance_sat: Option<u64>,
	client_balance_sat: Option<u64>,
	// confirms_within_blocks: Option<u32>,
	// channel_expiry_blocks: Option<u32>,
	announce_channel: bool,
	order_id: Option<OrderId>,
	info: Option<ChannelInfo>,
}

impl CRchannel {
	pub fn new(id: u128, counterparty_node_id: PublicKey, user_id: Option<u128>) -> Self {
		Self {
			channel_id: id,
			user_id: user_id.unwrap_or(0),
			counterparty_node_id,
			state: ChannelState::InfoRequested,
			// supported_version: None,
			lsp_balance_sat: None,
			client_balance_sat: None,
			//confirms_within_blocks: None,
			//channel_expiry_blocks: None,
			//token: None,
			//created_at: None,
			//expires_at: None,
			announce_channel: false,
			order_id: None,
			info: None,
		}
	}
}

#[derive(Default)]
struct PeerState {
	channels_by_id: HashMap<u128, CRchannel>,
	request_to_cid: HashMap<RequestId, u128>,
	pending_orders: HashMap<RequestId, Order>,
}

impl PeerState {
	pub fn insert_channel(&mut self, channel_id: u128, channel: CRchannel) {
		self.channels_by_id.insert(channel_id, channel);
	}

	pub fn insert_request(&mut self, request_id: RequestId, channel_id: u128) {
		self.request_to_cid.insert(request_id, channel_id);
	}

	pub fn get_channel_in_state_for_request(
		&mut self, request_id: &RequestId, state: ChannelState,
	) -> Option<&mut CRchannel> {
		let channel_id = self.request_to_cid.remove(request_id)?;

		if let Some(channel) = self.channels_by_id.get_mut(&channel_id) {
			if channel.state == state {
				return Some(channel);
			}
		}
		None
	}

	pub fn remove_channel(&mut self, channel_id: u128) {
		self.channels_by_id.remove(&channel_id);
	}
}

pub struct CRManager<
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
	per_peer_state: RwLock<HashMap<PublicKey, Mutex<PeerState>>>,
	// required as LSP creates orderId for a channel
	channels_by_orderid: RwLock<HashMap<OrderId, CRchannel>>,
	//orders_by_orderid: RwLock<HashMap<OrderId, Order>>
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
	> CRManager<ES, Descriptor, L, RM, CM, OM, CMH, NS>
where
	ES::Target: EntropySource,
	L::Target: Logger,
	RM::Target: RoutingMessageHandler,
	CM::Target: ChannelMessageHandler,
	OM::Target: OnionMessageHandler,
	CMH::Target: CustomMessageHandler,
	NS::Target: NodeSigner,
{
	pub fn new(
		entropy_source: ES, promise_secret: [u8; 32],
		pending_messages: Arc<Mutex<Vec<(PublicKey, LSPSMessage)>>>,
		pending_events: Arc<Mutex<Vec<Event>>>,
	) -> Self {
		Self {
			entropy_source,
			pending_messages,
			pending_events,
			per_peer_state: RwLock::new(HashMap::new()),
			channels_by_orderid: RwLock::new(HashMap::new()),
			peer_manager: Mutex::new(None),
		}
	}

	pub fn set_peer_manager(
		&self, peer_manager: Arc<PeerManager<Descriptor, CM, RM, OM, L, CMH, NS>>,
	) {
		*self.peer_manager.lock().unwrap() = Some(peer_manager);
	}

	fn connect_to_counterparty(&self, counterparty_node_id: PublicKey) {
		let channel_id = self.generate_channel_id();
		let channel = CRchannel::new(channel_id, counterparty_node_id, None);
		// Enqueue the info request message here
		let mut per_peer_state = self.per_peer_state.write().unwrap();
		let peer_state_mutex =
			per_peer_state.entry(counterparty_node_id).or_insert(Mutex::new(PeerState::default()));
		let peer_state = peer_state_mutex.get_mut().unwrap();
		peer_state.insert_channel(channel_id, channel);

		let request_id = self.generate_request_id();
		peer_state.insert_request(request_id.clone(), channel_id);

		{
			let mut pending_messages = self.pending_messages.lock().unwrap();
			pending_messages.push((
				counterparty_node_id,
				Message::Request(request_id, Request::GetInfo(GetInfoRequest {})).into(),
			));
		}

		if let Some(peer_manager) = self.peer_manager.lock().unwrap().as_ref() {
			peer_manager.process_events();
		}
	}

	fn handle_get_info_request(
		&self, request_id: RequestId, counterparty_node_id: PublicKey, options: OptionsSupported,
		website: &String,
	) {
		self.enqueue_response(
			counterparty_node_id,
			request_id,
			Response::GetInfo(GetInfoResponse {
				supported_versions: vec![1],
				website: *website,
				options,
			}),
		)
	}

	fn handle_get_info_response(
		&self, request_id: RequestId, counterparty_node_id: PublicKey, result: GetInfoResponse,
	) -> Result<(), LightningError> {
		// Change state to PEnding Selection if supported versions
		let per_peer_state = self.per_peer_state.read().unwrap();
		match per_peer_state.get(&counterparty_node_id) {
			Some(peer_state_mutex) => {
				let mut peer_state = peer_state_mutex.lock().unwrap();

				match peer_state
					.get_channel_in_state_for_request(&request_id, ChannelState::InfoRequested)
				{
					Some(channel) => {
						let channel_id = channel.channel_id;

						if result.supported_versions.contains(&SUPPORTED_SPEC_VERSION) {
							channel.state = ChannelState::OrderRequested;

							//let request_id = self.generate_request_id();
							// peer_state.insert_request(request_id.clone(), channel_id);

							self.enqueue_event(Event::LSPS1(super::event::Event::GetInfoResponse {
								channel_id,
								request_id,
								counterparty_node_id,
								version: result.supported_versions,
								website: result.website,
								options_supported: result.options,
							}))
						} else {
							peer_state.remove_channel(channel_id);
							return Err(LightningError {
								err: format!("Not supported versions: {:?}", request_id),
								action: ErrorAction::IgnoreAndLog(Level::Info),
							});
						}
					}
					None => {
						return Err(LightningError {
							err: format!(
								"Received get_info response without a matching channel: {:?}",
								request_id
							),
							action: ErrorAction::IgnoreAndLog(Level::Info),
						})
					}
				}
			}
			None => {
				return Err(LightningError {
					err: format!(
						"Received get_info response from unknown peer: {:?}",
						counterparty_node_id
					),
					action: ErrorAction::IgnoreAndLog(Level::Info),
				})
			}
		}

		Ok(())
	}

	fn handle_get_info_error(
		&self, request_id: RequestId, counterparty_node_id: &PublicKey, error: ResponseError,
	) -> Result<(), LightningError> {
		let per_peer_state = self.per_peer_state.read().unwrap();
		match per_peer_state.get(counterparty_node_id) {
			Some(peer_state_mutex) => {
				let mut peer_state = peer_state_mutex.lock().unwrap();

				match peer_state
					.get_channel_in_state_for_request(&request_id, ChannelState::OrderRequested)
				{
					Some(channel) => {
						let channel_id = channel.channel_id;
						peer_state.remove_channel(channel_id);
						return Err(LightningError {
							err: format!("Received error response from getinfo request ({:?}) with counterparty {:?}.  Removing channel {}. code = {}, message = {}", request_id, counterparty_node_id, channel_id, error.code, error.message),
							action: ErrorAction::IgnoreAndLog(Level::Info)
						});
					}
					None => {
						return Err(LightningError {
							err: format!("Received an unexpected error response for a getinfo request from counterparty ({:?})", counterparty_node_id),
							action: ErrorAction::IgnoreAndLog(Level::Info)
						});
					}
				}
			}
			None => {
				return Err(LightningError {
					err: format!("Received error response for a getinfo request from an unknown counterparty ({:?})", counterparty_node_id),
					action: ErrorAction::IgnoreAndLog(Level::Info)
				});
			}
		}
	}

	fn place_order(
		&self, counterparty_node_id: PublicKey, channel_id: u128, client_order: Order,
	) -> Result<(), APIError> {
		// Check all the conditions from the given GetInfoResponse
		// and then
		let per_peer_state = self.per_peer_state.read().unwrap();
		match per_peer_state.get(&counterparty_node_id) {
			Some(peer_state_mutex) => {
				let mut peer_state = peer_state_mutex.lock().unwrap();

				if let Some(channel) = peer_state.channels_by_id.get_mut(&channel_id) {
					if channel.state == ChannelState::OrderRequested {
						channel.state = ChannelState::PendingSelection;

						let request_id = self.generate_request_id();
						peer_state.insert_request(request_id.clone(), channel_id);
						{
							let mut pending_messages = self.pending_messages.lock().unwrap();
							pending_messages.push((
								counterparty_node_id,
								Message::Request(
									request_id,
									Request::CreateOrder(CreateOrderRequest {
										order: client_order,
									}),
								)
								.into(),
							))
						}
						if let Some(peer_manager) = self.peer_manager.lock().unwrap().as_ref() {
							peer_manager.process_events();
						}
					} else {
						return Err(APIError::APIMisuseError {
							err: "Channel is not pending selection".to_string(),
						});
					}
				}
			}
			None => {
				return Err(APIError::APIMisuseError {
					err: format!("Channel with id {} not found", channel_id),
				});
			}
		}

		Ok(())
	}

	// This function can be used both by client and LSP. For create order when client creates an order.
	// By LSP if create order request is valid.
	// Again by client in create order response.
	// To implement the error codes.
	fn is_valid_order(
		&self, counterparty_node_id: &PublicKey, order: &Order, options: &OptionsSupported,
	) -> Result<(), LightningError> {
		let per_peer_state = self.per_peer_state.read().unwrap();
		match per_peer_state.get(&counterparty_node_id) {
			Some(peer_state_mutex) => {
				let peer_state = peer_state_mutex.lock().unwrap();

				if let Some(channel) = peer_state.channels_by_id.get_mut(&channel_id) {
					if channel.state == ChannelState::OrderRequested {
						check_if_valid(
							order.lsp_balance_sat.into(),
							options.max_initial_lsp_balance_sat.into(),
							options.min_initial_lsp_balance_sat.into(),
						);

						check_if_valid(
							order.client_balance_sat.into(),
							options.max_initial_client_balance_sat.into(),
							options.min_initial_client_balance_sat.into(),
						);

						check_if_valid(
							order.channel_expiry_blocks.into(),
							options.max_channel_expiry_blocks.into(),
							0,
						);
					} else if channel.state == ChannelState::PendingPayment {
					}
				}
			}
			None => {}
		}
		// Check the condition for supported version

		// Other measures to implement
		// The client MUST check if option_support_large_channel is enabled
		// before they order a channel larger than 16,777,216 satoshi.

		// Token validity to check.

		Ok(())
	}

	// Enqueue the event of CreateChannel as LSP would need to calculate the fees.
	fn handle_create_order_request(
		&self, request_id: RequestId, counterparty_node_id: &PublicKey, request: CreateOrderRequest,
	) -> Result<(), LightningError> {
		// Guess no need to enforce this condition, will be implemented at higher abstraction
		// if self.is_valid_order(request.order, options) {
		let mut per_peer_state = self.per_peer_state.write().unwrap();
		let peer_state_mutex =
			per_peer_state.entry(*counterparty_node_id).or_insert(Mutex::new(PeerState::default()));
		let peer_state = peer_state_mutex.get_mut().unwrap();

		// clone the order or borrow it mutably
		// check validity here or create order id
		peer_state.pending_orders.insert(request_id.clone(), request.order.clone());

		self.enqueue_event(Event::LSPS1(super::event::Event::CreateInvoice {
			request_id,
			counterparty_node_id: *counterparty_node_id,
			order: request.order,
		}));

		Ok(())
	}

	// Check validity of order from the client and then respond with parameters for payment and channel object
	fn send_invoice_for_order(
		&self, request_id: RequestId, counterparty_node_id: &PublicKey, channel_id: u128,
	) -> Result<(), APIError> {
		let per_peer_state = self.per_peer_state.read().unwrap();
		match per_peer_state.get(counterparty_node_id) {
			Some(peer_state_mutex) => {
				let mut peer_state = peer_state_mutex.lock().unwrap();

				if let Some(channel) = peer_state.channels_by_id.get(&channel_id) {
					match peer_state.pending_orders.get_mut(&request_id) {
						Some(order) => {
							if let Some(order_id) = order.order_id {
								// already sent invoice for this order,
								// this is in pending order for status of payment confirmation
								return Err(APIError::APIMisuseError {
									err: format!(
										"Invoice already exists for this order with id {:?}",
										order_id
									),
								});
							}

							let order_id = self.generate_order_id();

							order.order_id = Some(order_id);
							order.order_state = OrderState::Created;
							channel.order_id = Some(order_id);

							// wrong- find which channel corresponds to this order_id
							// insert channels by order_id in peer manager; by LSP
							let mut channels_by_orderid = self.channels_by_orderid.write().unwrap();
							channels_by_orderid.insert(
								order_id,
								// The same channel is
								*channel.clone(),
							);

							// Change the arguments to this function and above
							let response = self.set_the_fees(&order);
							// Insert a channel in channel_by_orderid??
							// This done by LSP as it generates a orderid
							// this is required as CRchannelManager does not have another way of matching channels

							self.enqueue_response(
								*counterparty_node_id,
								request_id,
								Response::CreateOrder(response),
							);
						}
						None => {
							// not pending order
						}
					}
				} else {
					// wrong channel id
				}
			}
			None => {
				return Err(APIError::APIMisuseError {
					err: format!("Channel with id {} not found", channel_id),
				});
			}
		}
		Ok(())
	}

	fn set_the_fees(&self, request: &Order) -> CreateOrderResponse {

		// Give the LSP, parameters so that they can set the fees themselves
	}

	// Enqueue the PayforChannel event, to show client to pay for the LSP or abort.
	fn handle_create_order_response(
		&self, request_id: RequestId, counterparty_node_id: &PublicKey,
		response: &CreateOrderResponse,
	) -> Result<(), LightningError> {
		let per_peer_state = self.per_peer_state.read().unwrap();
		match per_peer_state.get(counterparty_node_id) {
			Some(peer_state_mutex) => {
				let mut peer_state = peer_state_mutex.lock().unwrap();

				self.enqueue_event(Event::LSPS1(super::event::Event::PayforChannel {
					request_id,
					counterparty_node_id: *counterparty_node_id,
					order: response.order,
					payment: response.payment,
					channel: response.channel,
				}));
			}
			None => {}
		}
		Ok(())
	}

	fn handle_create_order_error(
		&self, request_id: RequestId, counterparty_node_id: &PublicKey, error: ResponseError,
	) -> Result<(), LightningError> {
		let per_peer_state = self.per_peer_state.read().unwrap();
		match per_peer_state.get(counterparty_node_id) {
			Some(peer_state_mutex) => {
				let mut peer_state = peer_state_mutex.lock().unwrap();

				match peer_state
					.get_channel_in_state_for_request(&request_id, ChannelState::PendingSelection)
				{
					Some(channel) => {
						let channel_id = channel.channel_id;
						peer_state.remove_channel(channel_id);
						return Err(LightningError { err: format!( "Received error response from create order request ({:?}) with counterparty {:?}.  Removing channel {}. code = {}, message = {}", request_id, counterparty_node_id, channel_id, error.code, error.message), action: ErrorAction::IgnoreAndLog(Level::Info)});
					}
					None => {
						return Err(LightningError { err: format!("Received an unexpected error response for a create order request from counterparty ({:?})", counterparty_node_id), action: ErrorAction::IgnoreAndLog(Level::Info)});
					}
				}
			}
			None => {
				return Err(LightningError { err: format!("Received error response for a create order request from an unknown counterparty ({:?})",counterparty_node_id), action: ErrorAction::IgnoreAndLog(Level::Info)});
			}
		}
	}

	// Separate function then is_valid_order, just to abort the flow if fees higher than acceptable.
	// SHOULD validate the fee_total_sat is reasonable.
	// SHOULD validate fee_total_sat + client_balance_sat = order_total_sat.
	// MAY abort the flow here
	// In create_order response, the parameters given by the client should be matched with
	// that given by LSP. Although this might not be essential but still as a check.
	// Then this can also be included in the above function but implementing here would be good to abort
	// the flow. The fee rate throw high rate error and abort giving separate message.
	fn check_fees(
		&self, counterparty_node_id: PublicKey, channel_id: u128, client_order: Order,
		max_fees: u64, response: &CreateOrderResponse,
	) -> Result<u128, APIError> {
		let per_peer_state = self.per_peer_state.read().unwrap();
		match per_peer_state.get(&counterparty_node_id) {
			Some(peer_state_mutex) => {
				let mut peer_state = peer_state_mutex.lock().unwrap();

				if let Some(channel) = peer_state.channels_by_id.get(&channel_id) {
					if channel.state == ChannelState::PendingSelection {
						let total_fees =
							response.payment.fee_total_sat + response.order.client_balance_sat;

						// Other measures should also be included like confirms within blocks and channel
						// expiry blocks, both affect the fee rate
						if total_fees == response.payment.order_total_sat && total_fees < max_fees {
							return Ok(channel_id);
						} else {
							peer_state.remove_channel(channel_id);
							return Err(APIError::APIMisuseError {
								err: format!(
									"No pending buy request for request_id: {:?}",
									request_id
								),
							});
						}
					} else {
						return Err(APIError::APIMisuseError {
							err: "Channel is not pending selection".to_string(),
						});
					}
				} else {
					return Err(APIError::APIMisuseError {
						err: format!("Channel with id {} not found", channel_id),
					});
				}
			}
			None => {
				return Err(APIError::APIMisuseError {
					err: format!(
						"No state for the counterparty exists: {:?}",
						counterparty_node_id
					),
				});
			}
		}
	}

	fn liquidity_source_selected(
		&self, counterparty_node_id: PublicKey, request_id: RequestId,
		response: CreateOrderResponse,
	) -> Result<(), APIError> {
		let per_peer_state = self.per_peer_state.read().unwrap();
		match per_peer_state.get(&counterparty_node_id) {
			Some(peer_state_mutex) => {
				let mut peer_state = peer_state_mutex.lock().unwrap();

				match peer_state
					.get_channel_in_state_for_request(&request_id, ChannelState::PendingSelection)
				{
					Some(channel) => {
						channel.state = ChannelState::PendingPayment;
						channel.lsp_balance_sat = Some(response.order.lsp_balance_sat);
						channel.client_balance_sat = Some(response.order.client_balance_sat);
						// channel.token = order.token;
						//Set the refund address to self channel.refund_onchain_address;
						channel.announce_channel = response.order.announce_channel;

						self.enqueue_event(event::Event::PaymentforChannel {
							request_id,
							counterparty_node_id,
							order: response.order,
						});
					}
					None => {
						return Err(APIError::APIMisuseError {
							err: "Channel is not pending selection".to_string(),
						});
					}
				}
			}
			None => {
				return Err(APIError::APIMisuseError {
					err: format!("Channel with id {} not found", channel_id),
				});
			}
		}
		Ok(())
	}

	// This function just shows the payment invoice containing lightning address and onchain address to pay.
	fn pay_for_channel(
		&self, order: CreateOrderResponse, counterparty_node_id: &PublicKey, request_id: RequestId,
		channel_id: u128,
	) -> Result<&Payment, APIError> {
		let per_peer_state = self.per_peer_state.read().unwrap();
		match per_peer_state.get(&counterparty_node_id) {
			Some(peer_state_mutex) => {
				let mut peer_state = peer_state_mutex.lock().unwrap();

				if let Some(channel) = peer_state.channels_by_id.get(&channel_id) {
					if channel.state == ChannelState::PendingPayment {
						if order.payment.state == PaymentState::ExpectPayment {
							return Ok(&order.payment);
						} else {
							return Err(APIError::APIMisuseError {
								err: "The order is not expecting payment".to_string(),
							});
						}
					} else {
						return Err(APIError::APIMisuseError {
							err: "Channel is not pending selection".to_string(),
						});
					}
				}
			}
			None => {}
		}
		Ok(())
	}

	// Instead of below functions, give the user complete payment object
	// The user can then either complete payment onchain or offchain as per his convenience
	// Although in update order status fn, the user should either mention onchain payment done
	// or lighting payment done
	// This would not require setting a constant for setting preferred payment option.

	// Enqueue the event of updatepayment status after payment done
	fn onchain_payment() {

		// return the onchain payment adddress for the user to pay and other
		// parameters regarding block confirmation
	}

	// Enqueue the event of updatepayment status after payment done
	fn lighnting_payment() {

		// return lightning invoice for the user to pay and other parameters regarding
	}

	// user calls this to show that payment is done, with a few paramaters
	// Not sure about other parameters
	fn update_payment_status(
		&self, counterparty_node_id: &PublicKey, payment: &Payment, channel_id: u128, order: Order,
	) {
		let per_peer_state = self.per_peer_state.read().unwrap();
		match per_peer_state.get(&counterparty_node_id) {
			Some(peer_state_mutex) => {
				let mut peer_state = peer_state_mutex.lock().unwrap();

				if let Some(channel) = peer_state.channels_by_id.get(&channel_id) {
					self.enqueue_event(UpdatePaymentStatus {});
				}
			}
		}
	}

	// Send a request to get the order status
	fn check_order_status(
		self, channel_id: u128, counterparty_node_id: &PublicKey, order: &Order,
	) -> Result<(), APIError> {
		let per_peer_state = self.per_peer_state.read().unwrap();
		match per_peer_state.get(&counterparty_node_id) {
			Some(peer_state_mutex) => {
				let mut peer_state = peer_state_mutex.lock().unwrap();

				if let Some(channel) = peer_state.channels_by_id.get(&channel_id) {
					// wrong, order_id generated by LSP not client
					let request_id = self.generate_request_id();
					peer_state.insert_request(request_id, channel_id);

					{
						let mut pending_messages = self.pending_messages.lock().unwrap();
						pending_messages.push((
							*counterparty_node_id,
							Message::Request(
								request_id,
								Request::GetOrder(GetOrderRequest {
									order_id: order.order_id.unwrap(),
								}),
							)
							.into(),
						));
					}
					if let Some(peer_manager) = self.peer_manager.lock().unwrap().as_ref() {
						peer_manager.process_events();
					}
				} else {
					return Err(APIError::APIMisuseError {
						err: format!("No such channel_id exists: {:?}", channel_id),
					});
				}
			}
			None => {
				return Err(APIError::APIMisuseError {
					err: format!(
						"No state for the counterparty exists: {:?}",
						counterparty_node_id
					),
				})
			}
		}
		Ok(())
	}

	fn handle_get_order_request(
		&self, request_id: RequestId, counterparty_node_id: PublicKey, params: GetOrderRequest,
	) -> Result<(), APIError> {
		let per_peer_state = self.per_peer_state.read().unwrap();
		match per_peer_state.get(&counterparty_node_id) {
			Some(peer_state_mutex) => {
				let mut peer_state = peer_state_mutex.lock().unwrap();

				match peer_state
					.get_channel_in_state_for_request(&request_id, ChannelState::PendingPayment)
				{
					Some(channel) => {
						// To continously poll for whether payment is confirmed in another function
						// Respond directly the current order parameters
						//
						// Find the order in pending_order- no the client sends the Getrequest so
						// order_id is embeddded in it

						let order_id = params.order_id;

						// Find the order corresponding to the order_id, need to save the order with order_id
						// in some field
						// Should find orderid with respect to order
						let order = peer_state.pending_orders.get(&request_id);
						self.enqueue_response(
							counterparty_node_id,
							request_id,
							Response::GetOrder(GetOrderResponse { order }),
						)
					}
					None => {}
				}
			}
			None => {}
		}
		Ok(())
	}

	// Just to show the client about the status, no event or change in state
	fn handle_get_order_response(&self, request_id: RequestId, counterparty_node_id: &PublicKey) {

		// Check for different conditions
		// If payment is confirmed or refund is initiated
	}

	fn channel_ready() {}

	// Continoulsy poll for onchain confirmation to check if order is updated
	fn update_order_status() {}

	//
	fn channel_error() {}

	fn enqueue_response(
		&self, counterparty_node_id: PublicKey, request_id: RequestId, response: Response,
	) {
		{
			let mut pending_messages = self.pending_messages.lock().unwrap();
			pending_messages
				.push((counterparty_node_id, Message::Response(request_id, response).into()));
		}

		if let Some(peer_manager) = self.peer_manager.lock().unwrap().as_ref() {
			peer_manager.process_events();
		}
	}

	fn enqueue_event(&self, event: Event) {
		let mut pending_events = self.pending_events.lock().unwrap();
		pending_events.push(event);
	}

	fn generate_channel_id(&self) -> u128 {
		let bytes = self.entropy_source.get_secure_random_bytes();
		let mut id_bytes: [u8; 16] = [0; 16];
		id_bytes.copy_from_slice(&bytes[0..16]);
		u128::from_be_bytes(id_bytes)
	}

	fn generate_request_id(&self) -> RequestId {
		let bytes = self.entropy_source.get_secure_random_bytes();
		RequestId(utils::hex_str(&bytes[0..16]))
	}

	fn generate_order_id(&self) -> OrderId {
		let bytes = self.entropy_source.get_secure_random_bytes();
		OrderId(utils::hex_str(&bytes[0..16]))
	}
}

// Order of functions called
// new
// set peer
// connect to counterparty
// handle get info request
// handle get info response
// info error
// is valid order
// place order
// handle create order request
// send invoice for order
// set the fees
// handle create order response
// order error
// check fees
// liquidity source selected
// pay for channel
// onchain or lightning
// update payment status
// check order status
// handle get order request
// handle get order response
// open channel
// refund
