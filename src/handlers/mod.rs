pub mod iopub {
    use std::sync::mpsc;

    use crate::jupyter::messages::multipart::Multipart;
    use crate::IopubSocket;

    pub async fn handle(mut socket: IopubSocket, iopub_rx: mpsc::Receiver<Multipart>) {
        loop {
            let multipart = iopub_rx.recv().unwrap();
            multipart.send(&mut socket).await.unwrap();
        }
    }
}

pub mod heartbeat {
    use zeromq::{SocketRecv, SocketSend};

    use crate::HeartbeatSocket;

    pub async fn handle(mut socket: HeartbeatSocket) {
        loop {
            let msg = socket.recv().await.unwrap();
            socket.send(msg).await.unwrap();
        }
    }
}

pub mod shell;
pub mod stream;
