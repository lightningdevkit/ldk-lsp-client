use crate::channel_request;

/// An Event which you should probably take some action in response to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
	/// A LSPS1 (JIT Channel) protocol event
	LSPS1(channel_request::event::Event),
}
