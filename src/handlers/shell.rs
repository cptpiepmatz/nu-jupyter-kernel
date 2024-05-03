use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;

use mime::Mime;
use nu_protocol::engine::{EngineState, Stack, StateWorkingSet};
use parking_lot::Mutex;
use serde_json::json;
use zmq::Socket;

use crate::jupyter::messages::iopub::{self, ExecuteResult, IopubBroacast, Status};
use crate::jupyter::messages::multipart::Multipart;
use crate::jupyter::messages::shell::{
    ExecuteReply, ExecuteRequest, KernelInfoReply, ShellReply, ShellReplyOk, ShellRequest,
};
use crate::jupyter::messages::{Header, Message, Metadata};
use crate::nu::commands::external::External;
use crate::nu::render::{FormatDeclIds, PipelineRender};
use crate::nu::{self};

static EXECUTION_COUNTER: AtomicUsize = AtomicUsize::new(1);
static CELL_EVAL_COUNTER: AtomicUsize = AtomicUsize::new(1);
static RENDER_FILTER: Mutex<Option<Mime>> = Mutex::new(Option::None);

pub fn handle(
    socket: Socket,
    iopub: mpsc::Sender<Multipart>,
    mut engine_state: EngineState,
    format_decl_ids: FormatDeclIds,
) {
    let mut stack = Stack::new();
    loop {
        let message = match Message::recv(&socket) {
            Err(_) => {
                eprintln!("could not recv message");
                continue;
            }
            Ok(message) => message,
        };

        iopub
            .send(
                Status::Busy
                    .into_message(message.header.clone())
                    .into_multipart()
                    .unwrap(),
            )
            .unwrap();

        match &message.content {
            ShellRequest::Execute(request) => handle_execute_request(
                &socket,
                &message,
                &iopub,
                request,
                &mut engine_state,
                &mut stack,
                format_decl_ids,
            ),
            ShellRequest::IsComplete(_) => todo!(),
            ShellRequest::KernelInfo => {
                let kernel_info = KernelInfoReply::get();
                let reply = ShellReply::Ok(ShellReplyOk::KernelInfo(kernel_info));
                let msg_type = ShellReply::msg_type(&message.header.msg_type).unwrap();
                let reply = Message {
                    zmq_identities: message.zmq_identities,
                    header: Header::new(msg_type),
                    parent_header: Some(message.header.clone()),
                    metadata: Metadata::empty(),
                    content: reply,
                    buffers: vec![],
                };
                reply.into_multipart().unwrap().send(&socket).unwrap();
            }
        }

        iopub
            .send(
                Status::Idle
                    .into_message(message.header)
                    .into_multipart()
                    .unwrap(),
            )
            .unwrap();
    }
}

fn handle_execute_request(
    socket: &Socket,
    message: &Message<ShellRequest>,
    iopub: &mpsc::Sender<Multipart>,
    request: &ExecuteRequest,
    engine_state: &mut EngineState,
    stack: &mut Stack,
    format_decl_ids: FormatDeclIds,
) {
    let ExecuteRequest {
        code,
        silent,
        store_history,
        user_expressions,
        allow_stdin,
        stop_on_error,
    } = request;
    let msg_type = ShellReply::msg_type(&message.header.msg_type).unwrap();
    External::apply(engine_state).unwrap();

    let cell_name = format!(
        "cell-{}-{}",
        EXECUTION_COUNTER.load(Ordering::SeqCst),
        CELL_EVAL_COUNTER.fetch_add(1, Ordering::SeqCst)
    );
    let pipeline_data = match nu::execute(&code, engine_state, stack, &cell_name) {
        Ok(data) => data,
        Err(e) => {
            let mut working_set = StateWorkingSet::new(engine_state);
            let report = match e {
                nu::ExecuteError::Parse { ref error, delta } => {
                    working_set.delta = delta;
                    error as &(dyn miette::Diagnostic + Send + Sync + 'static)
                }
                nu::ExecuteError::Shell(ref error) => {
                    error as &(dyn miette::Diagnostic + Send + Sync + 'static)
                }
            };

            let name = report
                .code()
                .unwrap_or_else(|| Box::new(format_args!("nu-jupyter-kernel::unknown-error")));
            let value = nu_protocol::format_error(&working_set, report);
            // TODO: for traceback use error source
            let traceback = vec![];

            // we send display data to have control over the rendering of the output
            let broadcast = IopubBroacast::DisplayData(iopub::DisplayData {
                data: HashMap::from([(mime::TEXT_PLAIN.to_string(), value.clone())]),
                metadata: HashMap::new(),
                transient: HashMap::new(),
            });
            let broadcast = Message {
                zmq_identities: message.zmq_identities.clone(),
                header: Header::new(broadcast.msg_type()),
                parent_header: Some(message.header.clone()),
                metadata: Metadata::empty(),
                content: broadcast,
                buffers: vec![],
            };
            iopub.send(broadcast.into_multipart().unwrap()).unwrap();

            let reply = ShellReply::Error {
                name: name.to_string(),
                value,
                traceback,
            };
            let reply = Message {
                zmq_identities: message.zmq_identities.clone(),
                header: Header::new(msg_type),
                parent_header: Some(message.header.clone()),
                metadata: Metadata::empty(),
                content: reply,
                buffers: vec![],
            };
            reply.into_multipart().unwrap().send(socket).unwrap();
            return;
        }
    };

    let execution_count = EXECUTION_COUNTER.fetch_add(1, Ordering::SeqCst);
    // reset eval counter to keep last digit of cell names as tries for that cell
    CELL_EVAL_COUNTER.store(1, Ordering::SeqCst);

    if !pipeline_data.is_nothing() {
        let mut render_filter = RENDER_FILTER.lock();
        let render = PipelineRender::render(
            pipeline_data,
            engine_state,
            stack,
            format_decl_ids,
            render_filter.take(),
        );

        let execute_result = ExecuteResult {
            execution_count,
            data: render
                .data
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
            metadata: render
                .metadata
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
        };
        let broadcast = IopubBroacast::from(execute_result);
        let broadcast = Message {
            zmq_identities: message.zmq_identities.clone(),
            header: Header::new(broadcast.msg_type()),
            parent_header: Some(message.header.clone()),
            metadata: Metadata::empty(),
            content: broadcast,
            buffers: vec![],
        };
        iopub.send(broadcast.into_multipart().unwrap()).unwrap();
    }

    let reply = ExecuteReply {
        execution_count,
        user_expressions: json!({}),
    };
    let reply = ShellReply::Ok(ShellReplyOk::Execute(reply));
    let reply = Message {
        zmq_identities: message.zmq_identities.clone(),
        header: Header::new(msg_type),
        parent_header: Some(message.header.clone()),
        metadata: Metadata::empty(),
        content: reply,
        buffers: vec![],
    };
    reply.into_multipart().unwrap().send(&socket).unwrap();
}
