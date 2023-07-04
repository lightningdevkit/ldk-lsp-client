use bitcoin::secp256k1::PublicKey;

use crate::transport::msgs::RequestId;
use super::msgs::{OptionsSupported, ChannelInfo, Order, Payment};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    
    // Information from the LSP regarding fees and channel parameters.
    // The client should validate if the LSP's parameters supported match 
    // requirements.
    GetInfoResponse {
        channel_id: u128,
        
        request_id: RequestId,
    
		/// The node id of the LSP that provided this response.
		counterparty_node_id: PublicKey,

        version: Vec<u16>,

        website: String,

        options_supported: OptionsSupported
    },

    // Channel opening request from the client after selecting the LSP with desired parameters.
    // Client selects the fee and channel parameters and requests the LSP to create a channel.
    // LSP should check the validity of the create channel order.
    CreateInvoice {
        request_id: RequestId,

		counterparty_node_id: PublicKey,

        order: Order,
    },

    // LSP accepts the request parameters and sends an onchain address and invoice along with channel
    // parameters to the client. After payment by the client this event should be updated,
    // to show the LSP to poll for the payment now.
    PayforChannel {
        request_id: RequestId,
		counterparty_node_id: PublicKey,
		order: Order,
		payment: Payment,
		channel: Option<ChannelInfo>,
    },


    UpdatePaymentStatus {},

    // On payment confirmation, channel is opened. After payment confirms,
    // LSP should open a channel and open to client.
    OpenChannel {},

    // If order fails, refund is initiated.
    // 
    Refund {},
}