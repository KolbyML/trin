use std::sync::{atomic::Ordering, Arc};

use std::io;
use tokio::sync::Semaphore;
use utp_rs::{cid::ConnectionId, conn::ConnectionConfig, socket::UtpSocket, stream::UtpStream};

/// uTP Transfer Direction
#[derive(Debug, Clone, Copy)]
pub enum UtpDirection {
    /// uTP transfers initiated by a peer
    Inbound,
    /// uTP transfers initiated by the node
    Outbound,
}

pub struct UtpController {
    transfers_per_content_id_limit: usize,
    pub inbound_utp_transfer_semaphore: Arc<Semaphore>,
    pub outbound_utp_transfer_semaphore: Arc<Semaphore>,
    /// uTP socket.
    utp_socket: UtpSocket<crate::discovery::UtpEnr>,
}

impl UtpController {
    pub async fn new(
        inbound_utp_transfer_limit: usize,
        outbound_utp_transfer_limit: usize,
        transfers_per_content_id_limit: usize,
        utp_socket: UtpSocket<crate::discovery::UtpEnr>,
    ) -> Self {
        Self {
            transfers_per_content_id_limit,
            utp_socket,
            inbound_utp_transfer_semaphore: Arc::new(Semaphore::new(inbound_utp_transfer_limit)),
            outbound_utp_transfer_semaphore: Arc::new(Semaphore::new(outbound_utp_transfer_limit)),
        }
    }

    pub async fn connect_with_cid(
        &self,
        cid: ConnectionId<crate::discovery::UtpEnr>,
        config: ConnectionConfig,
    ) -> io::Result<UtpStream<crate::discovery::UtpEnr>> {
        self.utp_socket.connect_with_cid(cid, config).await
    }
    pub async fn accept_with_cid(
        &self,
        cid: ConnectionId<crate::discovery::UtpEnr>,
        config: ConnectionConfig,
    ) -> io::Result<UtpStream<crate::discovery::UtpEnr>> {
        self.utp_socket.accept_with_cid(cid, config).await
    }
    pub fn cid(
        &self,
        peer: crate::discovery::UtpEnr,
        is_initiator: bool,
    ) -> ConnectionId<crate::discovery::UtpEnr> {
        self.utp_socket.cid(peer, is_initiator)
    }
}
