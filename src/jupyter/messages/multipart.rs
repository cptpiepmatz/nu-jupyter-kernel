use std::iter;

use hmac::Mac;
use zmq::Socket;

use super::{Message, OutgoingContent, DIGESTER};

pub struct Multipart(Box<dyn Iterator<Item = zmq::Message> + Send>);

impl Multipart{
    pub fn send(self, socket: &Socket) -> Result<(), ()> {
        socket.send_multipart(self.0, 0).unwrap();
        Ok(())
    }
}

impl Message<OutgoingContent> {
    fn into_multipart_impl(self) -> Result<Multipart, ()> {
        let zmq_identities = self
            .zmq_identities
            .into_iter()
            .map(|bytes| Vec::from(bytes));
        let header = serde_json::to_string(&self.header).unwrap();
        let parent_header = match self.parent_header {
            Some(ref parent_header) => serde_json::to_string(parent_header).unwrap(),
            None => "{}".to_owned(),
        };
        let metadata = serde_json::to_string(&self.metadata).unwrap();
        let content = match self.content {
            OutgoingContent::Shell(ref content) => serde_json::to_string(content).unwrap(),
            OutgoingContent::Iopub(ref content) => serde_json::to_string(content).unwrap(),
        };
        let buffers = self.buffers.into_iter().map(|bytes| Vec::from(bytes));

        let mut digester = DIGESTER.get().clone();
        digester.update(header.as_bytes());
        digester.update(parent_header.as_bytes());
        digester.update(metadata.as_bytes());
        digester.update(content.as_bytes());
        let signature = digester.finalize().into_bytes();
        let signature = hex::encode(signature);

        let iter = zmq_identities
            .map(zmq::Message::from)
            .chain(iter::once(zmq::Message::from(b"<IDS|MSG>".as_slice())))
            .chain(
                [signature, header, parent_header, metadata, content]
                    .into_iter()
                    .map(String::into_bytes)
                    .map(zmq::Message::from),
            )
            .chain(buffers.map(zmq::Message::from));

        Ok(Multipart(Box::new(iter)))
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
