use std::sync::Arc;

use utp_rs::socket::UtpSocket;

pub struct UtpController {
    inbound_utp_transfer_limit: usize,
    outbound_utp_transfer_limit: usize,
    /// uTP socket.
    utp_socket: UtpSocket<crate::discovery::UtpEnr>,
}

impl UtpController {
    pub async fn new(
        inbound_utp_transfer_limit: usize,
        outbound_utp_transfer_limit: usize,
        utp_socket: UtpSocket<crate::discovery::UtpEnr>,
    ) -> Self {
        Self {
            inbound_utp_transfer_limit,
            outbound_utp_transfer_limit,
            utp_socket,
        }
    }
}
