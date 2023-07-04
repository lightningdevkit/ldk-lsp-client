
use std::convert::TryFrom;

use serde::{Deserialize, Serialize};

//use bitcoin::hashes::hmac::{Hmac, HmacEngine};
//use bitcoin::hashes::sha256::Hash as Sha256;
//use bitcoin::hashes::{Hash, HashEngine};
use crate::transport::msgs::{RequestId, ResponseError, LSPSMessage};
use crate::utils;

pub(crate) const LSPS1_GETINFO_METHOD_NAME: &str = "lsps1.getinfo";
pub(crate) const LSPS1_CREATE_ORDER_METHOD_NAME: &str = "lsps1.create_order";
pub(crate) const LSPS1_GET_ORDER_METHOD_NAME: &str = "lsps1.get_order";


pub(crate) const REFUND_ONCHAIN_ADDRESS: bool = false;

// Create a const to show preferred way for user payment
// Should this be set everytime before payment?
// Ask user for lighting or onchain and then set the const to
// lightning or onchain


#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Hash)]
pub struct OrderId(pub String);


#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct GetInfoRequest {}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct OptionsSupported {
	pub minimum_channel_confirmations: u8,
	pub minimum_onchain_payment_confirmations: u8,
	pub supports_zero_channel_reserve: bool,
	pub min_onchain_payment_size_sat: Option<u32>,
	pub max_channel_expiry_blocks: u32,
	pub min_initial_client_balance_sat: u64,
    pub max_initial_client_balance_sat: u64,
    pub min_initial_lsp_balance_sat: u64,
    pub max_initial_lsp_balance_sat: u64,
    pub min_channel_balance_sat: u64,
    pub max_channel_balance_sat: u64,
}


#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct GetInfoResponse {
    pub supported_versions: Vec<u16>,
    pub website: String,
	pub options: OptionsSupported,
}


#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct CreateOrderRequest {
	pub order: Order,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Order {
	pub order_id: Option<OrderId>,
	pub api_version: u16,
	pub lsp_balance_sat: u64,
	pub client_balance_sat: u64,
	pub confirms_within_blocks: u32,
	pub channel_expiry_blocks: u32,
	pub token: String,
	pub announce_channel: bool,
	pub refund_onchain_address: Option<String>,
	pub order_state: OrderState,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct CreateOrderResponse {
	pub order: Order,
	pub created_at: String,
	pub expires_at: String,
	pub payment: Payment,
	pub channel: Option<ChannelInfo>,
}

impl CreateOrderResponse {
	// import datetime and set to time to creaetd_at.
	pub fn new(order: &mut Order, fee: u64, bolt11_invoice: String,
	onchain_address: String, options: OptionsSupported) -> Self {
		// Few of the parameters are mirrored from the orderrequest.
	
		let response = CreateOrderResponse {
			order: request.order,
			payment,
			channel: None,
		};
		response
	}
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum OrderState {
	Requested,
	Created,
	Completed,
	Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Payment {
	pub state: PaymentState,
    pub fee_total_sat: u64,
    pub order_total_sat: u64,
    pub onchain_address: String,
	pub bolt11_invoice: String,
    pub onchain_block_confirmations_required: u8,
    pub minimum_fee_for_0conf: u8,
	pub onchain_payment: OnchainPayment,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum PaymentState{
	ExpectPayment,
	Hold,
	Paid,
	Refunded,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct OnchainPayment{
	pub outpoint: String,
	pub sat: u64,
	pub confirmed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ChannelInfo {
	pub state: ChannelStatus,
	pub funded_at: String,
	pub funding_outpoint: String,
	pub scid: Option<String>,
	pub expires_at: String,
	pub closing_transaction: Option<String>,
	pub closed_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum ChannelStatus {
	Opening,
	Opened,
	Closed,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct GetOrderRequest {
	pub order_id: OrderId,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct GetOrderResponse {
	pub response: Order,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
	GetInfo(GetInfoRequest),
	CreateOrder(CreateOrderRequest),
	GetOrder(GetOrderRequest),
}

impl Request {
	pub fn method(&self) -> &str {
		match self {
			Request::GetInfo(_) => LSPS1_GETINFO_METHOD_NAME,
			Request::CreateOrder(_) => LSPS1_CREATE_ORDER_METHOD_NAME,
			Request::GetOrder(_) => LSPS1_GET_ORDER_METHOD_NAME,
		}
	}
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Response {
	GetInfo(GetInfoResponse),
	GetInfoError(ResponseError),
	CreateOrder(CreateOrderResponse),
	OrderError(ResponseError),
	GetOrder(GetOrderResponse),
	GetOrderError(ResponseError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
	Request(RequestId, Request),
	Response(RequestId, Response),
}

impl TryFrom<LSPSMessage> for Message {
	type Error = ();

	fn try_from(message: LSPSMessage) -> Result<Self, Self::Error> {
		if let LSPSMessage::LSPS1(message) = message {
			return Ok(message);
		}

		Err(())
	}
}

impl From<Message> for LSPSMessage {
	fn from(message: Message) -> Self {
		LSPSMessage::LSPS1(message)
	}
}