use std::iter;

use bytes::Bytes;
use hmac::Mac;
use zeromq::{SocketSend, ZmqMessage};

use super::{Message, OutgoingContent, DIGESTER};

pub struct Multipart(ZmqMessage);

impl Multipart {
    pub async fn send<S: SocketSend>(self, socket: &mut S) -> Result<(), ()> {
        socket.send(self.0).await.unwrap();
        Ok(())
    }
}

impl Message<OutgoingContent> {
    fn into_multipart_impl(self) -> Result<Multipart, ()> {
        let zmq_identities = self.zmq_identities;
        let header = serde_json::to_string(&self.header).unwrap();
        let parent_header = match self.parent_header {
            Some(ref parent_header) => serde_json::to_string(parent_header).unwrap(),
            None => "{}".to_owned(),
        };
        let metadata = serde_json::to_string(&self.metadata).unwrap();
        let content = match self.content {
            OutgoingContent::Shell(ref content) => serde_json::to_string(content).unwrap(),
            OutgoingContent::Iopub(ref content) => serde_json::to_string(content).unwrap(),
            OutgoingContent::Control(ref content) => serde_json::to_string(content).unwrap(),
        };
        let buffers = self.buffers;

        let mut digester = DIGESTER.get().clone();
        digester.update(header.as_bytes());
        digester.update(parent_header.as_bytes());
        digester.update(metadata.as_bytes());
        digester.update(content.as_bytes());
        let signature = digester.finalize().into_bytes();
        let signature = hex::encode(signature);

        let frames: Vec<Bytes> = zmq_identities
            .into_iter()
            .chain(iter::once(Bytes::from_static(b"<IDS|MSG>")))
            .chain(
                [signature, header, parent_header, metadata, content]
                    .into_iter()
                    .map(Bytes::from),
            )
            .chain(buffers)
            .collect();

        Ok(Multipart(ZmqMessage::try_from(frames).unwrap()))
    }
}

impl<C> Message<C>
where
    C: Into<OutgoingContent>,
{
    pub fn into_multipart(self) -> Result<Multipart, ()> {
        let Message {
            zmq_identities,
            header,
            parent_header,
            metadata,
            content,
            buffers,
        } = self;
        let msg = Message {
            zmq_identities,
            header,
            parent_header,
            metadata,
            content: content.into(),
            buffers,
        };
        msg.into_multipart_impl()
    }
}
