pub mod iopub {
    use std::sync::mpsc;

    use zmq::Socket;

    use crate::jupyter::messages::multipart::Multipart;

    pub fn handle(socket: Socket, iopub_rx: mpsc::Receiver<Multipart>) {
        loop {
            let multipart = iopub_rx.recv().unwrap();
            multipart.send(&socket).unwrap();
        }
    }
}

pub mod heartbeat {
    use zmq::Socket;

    pub fn handle(socket: Socket) {
        loop {
            let msg = socket.recv_multipart(0).unwrap();
            socket.send_multipart(msg, 0).unwrap();
        }
    }
}

pub mod shell;
pub mod stream;
