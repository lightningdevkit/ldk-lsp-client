
use serde::{Deserialize, Serialize};

//use bitcoin::hashes::hmac::{Hmac, HmacEngine};
//use bitcoin::hashes::sha256::Hash as Sha256;
//use bitcoin::hashes::{Hash, HashEngine};
use crate::transport::msgs::{RequestId, ResponseError};
use crate::utils;

pub(crate) const LSPS1_GETINFO_METHOD_NAME: &str = "lsps1.getinfo";
pub(crate) const LSPS1_CREATE_ORDER_METHOD_NAME: &str = "lsps1.create_order";
pub(crate) const LSPS1_GET_ORDER_METHOD_NAME: &str = "lsps1.get_order";

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
	pub options: Vec<OptionsSupported>,
}

impl GetInfoResponse {
	fn is_valid(&self) -> bool {
		// TODO check validity of min < max for every pair 
		// Use of hmacengine/hash for checking
		1
	}
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct CreateOrderRequest {
	pub api_version: u8,
	pub lsp_balance_sat: String,
	pub client_balance_sat: String,
	pub confirms_within_blocks: u8,
	pub channel_expiry_blocks: u8,
	pub token: String,
	// String of Onchain address, maybe object- String/bech32,
	pub refund_onchain_address: String,
	pub announce_channel: bool,
}

impl CreateOrderRequest {
	fn is_valid(&self, req: &GetInfoResponse) -> bool {
		// implement the conditions
	}
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Order {
	pub order_id: String,
	pub api_version: u16,
	pub lsp_balance_sat: u64,
	pub client_balance_sat: u64,
	pub confirms_within_blocks: u32,
	pub channel_expiry_blocks: u32,
	pub token: String,
	pub created_at: String,
	pub expires_at: String,
	pub announce_channel: bool,
	pub order_state: OrderState,
	pub payment: Payment,
	pub channel: Option<Channel>,
}


#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct CreateOrderResponse{
	pub response: Order,
}

impl CreateOrderResponse {
	fn new(request: &CreateOrderRequest) -> () {
		// Few of the parameters are mirrored from the orderrequest.
	}

	fn is_valid(&self) -> () {
		// Check the following conditions

	}
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum OrderState {
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
pub struct Channel {
	pub state: ChannelState,
	pub funded_at: String,
	pub funding_outpoint: String,
	pub scid: Option<String>,
	pub expires_at: String,
	pub closing_transaction: Option<String>,
	pub closed_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum ChannelState {
	Opening,
	Opened,
	Closed,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct GetOrderRequest {
	pub order_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct GetOrderResponse {
	pub response: OrderState,
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
