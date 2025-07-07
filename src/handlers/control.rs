use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::sync::broadcast;

use crate::ControlSocket;
use crate::jupyter::Shutdown;
use crate::jupyter::kernel_info::KernelInfo;
use crate::jupyter::messages::control::{ControlReply, ControlReplyOk, ControlRequest};
use crate::jupyter::messages::{Header, Message, Metadata};

pub async fn handle(
    mut socket: ControlSocket,
    shutdown_sender: broadcast::Sender<Shutdown>,
    interrupt_signal: Arc<AtomicBool>,
) {
    loop {
        let message = Message::<ControlRequest>::recv(&mut socket).await.unwrap();
        match &message.content {
            ControlRequest::KernelInfo => handle_kernel_info_request(&mut socket, &message).await,
            ControlRequest::Shutdown(shutdown) => {
                handle_shutdown_request(&mut socket, &message, *shutdown, &shutdown_sender).await;
                match shutdown.restart {
                    true => continue,
                    false => break,
                }
            }
            ControlRequest::Interrupt => {
                handle_interrupt_request(&mut socket, &message, interrupt_signal.deref()).await
            }
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

async fn handle_interrupt_request(
    socket: &mut ControlSocket,
    message: &Message<ControlRequest>,
    interrupt_signal: &AtomicBool,
) {
    interrupt_signal.store(true, Ordering::Relaxed);

    while interrupt_signal.load(Ordering::Relaxed) {
        // poll the interrupt signal to check when the engine is successfully
        // interrupted
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let reply = ControlReply::Ok(ControlReplyOk::Interrupt);
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
