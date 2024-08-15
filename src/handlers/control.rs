use tokio::sync::broadcast;

use crate::jupyter::kernel_info::KernelInfo;
use crate::jupyter::messages::control::{ControlReply, ControlReplyOk, ControlRequest};
use crate::jupyter::messages::{Header, Message, Metadata};
use crate::jupyter::Shutdown;
use crate::ControlSocket;

pub async fn handle(mut socket: ControlSocket, sender: broadcast::Sender<Shutdown>) {
    loop {
        let message = Message::<ControlRequest>::recv(&mut socket).await.unwrap();
        match &message.content {
            ControlRequest::KernelInfo => handle_kernel_info_request(&mut socket, &message).await,
            ControlRequest::Shutdown(shutdown) => {
                handle_shutdown_request(&mut socket, &message, *shutdown, &sender).await;
                match shutdown.restart {
                    true => continue,
                    false => break,
                }
            }
            ControlRequest::Interrupt => todo!(),
            ControlRequest::Debug => todo!(),
        }
    }
}

async fn handle_kernel_info_request(socket: &mut ControlSocket, message: &Message<ControlRequest>) {
    let kernel_info = KernelInfo::get();
    let reply = ControlReply::Ok(ControlReplyOk::KernelInfo(Box::new(kernel_info)));
    let msg_type = ControlReply::msg_type(&message.header.msg_type).unwrap();
    let reply = Message {
        zmq_identities: message.zmq_identities.clone(),
        header: Header::new(msg_type),
        parent_header: Some(message.header.clone()),
        metadata: Metadata::empty(),
        content: reply,
        buffers: vec![],
    };
    reply.into_multipart().unwrap().send(socket).await.unwrap();
}

async fn handle_shutdown_request(
    socket: &mut ControlSocket,
    message: &Message<ControlRequest>,
    shutdown: Shutdown,
    sender: &broadcast::Sender<Shutdown>,
) {
    // according to docs, we first shut our kernel and then reply to the client
    sender.send(shutdown).unwrap();

    // TODO: check if application terminated, maybe with broadcast that ensures that
    // every       subscriber received something
    let reply = ControlReply::Ok(ControlReplyOk::Shutdown(shutdown));
    let msg_type = ControlReply::msg_type(&message.header.msg_type).unwrap();
    let reply = Message {
        zmq_identities: message.zmq_identities.clone(),
        header: Header::new(msg_type),
        parent_header: Some(message.header.clone()),
        metadata: Metadata::empty(),
        content: reply,
        buffers: vec![],
    };
    reply.into_multipart().unwrap().send(socket).await.unwrap();
}
