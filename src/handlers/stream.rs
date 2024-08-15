use std::fs::File;
use std::io::{self, Read};
use std::os;
use std::sync::Arc;
use std::thread::{self};

use bytes::Bytes;
use parking_lot::Mutex;
use tokio::sync::mpsc;

use crate::jupyter::messages::iopub::{self, IopubBroacast};
use crate::jupyter::messages::multipart::Multipart;
use crate::jupyter::messages::{Header, Message, Metadata};

const BUFFER_SIZE: usize = 8 * 1024;

pub struct StreamHandler {
    message_data: Arc<Mutex<(Vec<Bytes>, Option<Header>)>>,
    stream_name: iopub::StreamName, // iopub_tx: Sender<Multipart>, // moved into the thread
}

impl StreamHandler {
    pub fn start(
        stream_name: iopub::StreamName,
        iopub_tx: mpsc::Sender<Multipart>,
    ) -> io::Result<(Self, File)> {
        // TODO: construct a Self, create a pipe, start a reader thread, return the
        // writer as a file
        let message_data = Arc::new(Mutex::new((vec![], None)));

        let (mut pipe_reader, pipe_writer) = os_pipe::pipe()?;
        let t_message_data = message_data.clone();
        thread::Builder::new()
            .name(format!("{} reader", stream_name.as_ref()))
            .spawn(move || {
                let mut read_buf = [0u8; BUFFER_SIZE];
                loop {
                    let mut s_buf: Vec<u8> = Vec::new();
                    loop {
                        match pipe_reader.read(&mut read_buf) {
                            Err(err) => todo!("handle that error"),
                            Ok(0) if s_buf.is_empty() => todo!("stream is dead"),
                            Ok(0) => break,
                            Ok(BUFFER_SIZE) => s_buf.extend_from_slice(&read_buf),
                            Ok(n) => {
                                s_buf.extend_from_slice(&read_buf[..n]);
                                break;
                            }
                        }
                    }
                    // TODO: handle this somehow
                    let s = String::from_utf8(s_buf).unwrap();
                    let broadcast = IopubBroacast::Stream(iopub::Stream {
                        name: stream_name,
                        text: s,
                    });
                    let (zmq_identities, parent_header) = t_message_data.lock().clone();
                    let message = Message {
                        zmq_identities,
                        header: Header::new(broadcast.msg_type()),
                        parent_header,
                        metadata: Metadata::empty(),
                        content: broadcast,
                        buffers: vec![],
                    };
                    // TODO: handle this better, also does this need to be blocking?
                    iopub_tx
                        .blocking_send(message.into_multipart().unwrap())
                        .unwrap();
                }
            })?;

        #[cfg(windows)]
        let file: File = os::windows::io::OwnedHandle::from(pipe_writer).into();
        #[cfg(unix)]
        let file: File = os::unix::io::OwnedFd::from(pipe_writer).into();

        Ok((
            Self {
                message_data,
                stream_name,
            },
            file,
        ))
    }

    pub fn update_reply(&mut self, zmq_identities: Vec<Bytes>, parent_header: Header) {
        let mut message_data = self.message_data.lock();
        message_data.0 = zmq_identities;
        message_data.1 = Some(parent_header);
    }
}
