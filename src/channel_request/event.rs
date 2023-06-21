

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    // Request from the client for server info.
    GetInfo {},

    // Information from the LSP regarding fees and channel parameters.
    GetInfoResponse {},

    // Channel opening request from the client after selecting the LSP with desired parameters.
    // Client selects the fee and channel parameters and requests the LSP to create a channel.
    CreateChannel {},

    // LSP accepts the request parameters and sends an onchain address and invoice along with channel
    // parameters to the client.
    Order {},

    // Client confirms the parameters and pays the LSP through onchain or lightning payment.
    PaymentforChannel {},

    // Waits for the payment to confirm and till then the order status can be retrived.
    GetOrderStatus {},

    // Checks whether the payment is confirmed and updates the order.
    ConfirmPayment {},

    // On payment confirmation, channel is opened.
    OpenChannel {},

    // If order fails, refund is initiated.
    Refund {},
}