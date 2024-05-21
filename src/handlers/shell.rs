use std::collections::HashMap;
use std::fs::File;
use std::os;
use std::sync::{mpsc, Arc};

use mime::Mime;
use nu_protocol::engine::{EngineState, Stack, StateWorkingSet};
use nu_protocol::PipelineData;
use parking_lot::Mutex;
use serde_json::json;
use zmq::Socket;

use super::stream::StreamHandler;
use crate::jupyter::messages::iopub::{self, ExecuteResult, IopubBroacast, Status};
use crate::jupyter::messages::multipart::Multipart;
use crate::jupyter::messages::shell::{
    ExecuteReply, ExecuteRequest, KernelInfoReply, ShellReply, ShellReplyOk, ShellRequest,
};
use crate::jupyter::messages::{Header, Message, Metadata};
use crate::nu::commands::external::External;
use crate::nu::konst::Konst;
use crate::nu::render::{FormatDeclIds, PipelineRender, StringifiedPipelineRender};
use crate::nu::{self, ExecuteError};

// TODO: get rid of this static by passing this into the display command
pub static RENDER_FILTER: Mutex<Option<Mime>> = Mutex::new(Option::None);

pub fn handle(
    socket: Socket,
    iopub: mpsc::Sender<Multipart>,
    mut stdout_handler: StreamHandler,
    mut stderr_handler: StreamHandler,
    mut engine_state: EngineState,
    mut stack: Stack,
    format_decl_ids: FormatDeclIds,
    konst: Konst,
    mut cell: Cell,
) {
    loop {
        let message = match Message::recv(&socket) {
            Err(_) => {
                eprintln!("could not recv message");
                continue;
            }
            Ok(message) => message,
        };

        let mut ctx = HandlerContext {
            socket: &socket,
            iopub: &iopub,
            stdout_handler: &mut stdout_handler,
            stderr_handler: &mut stderr_handler,
            engine_state: &mut engine_state,
            format_decl_ids,
            konst,
            stack: &mut stack,
            cell: &mut cell,
            message: &message,
        };

        send_status(&ctx, Status::Busy);

        match &message.content {
            ShellRequest::Execute(request) => handle_execute_request(&mut ctx, request),
            ShellRequest::IsComplete(_) => todo!(),
            ShellRequest::KernelInfo => handle_kernel_info_request(&ctx),
        }

        send_status(&ctx, Status::Idle);
    }
}

struct HandlerContext<'so, 'io, 'soh, 'seh, 'es, 'st, 'c, 'm> {
    socket: &'so Socket,
    iopub: &'io mpsc::Sender<Multipart>,
    stdout_handler: &'soh mut StreamHandler,
    stderr_handler: &'seh mut StreamHandler,
    engine_state: &'es mut EngineState,
    format_decl_ids: FormatDeclIds,
    konst: Konst,
    stack: &'st mut Stack,
    cell: &'c mut Cell,
    message: &'m Message<ShellRequest>,
}

fn send_status(ctx: &HandlerContext, status: Status) {
    ctx.iopub
        .send(
            status
                .into_message(ctx.message.header.clone())
                .into_multipart()
                .unwrap(),
        )
        .unwrap();
}

fn handle_kernel_info_request(ctx: &HandlerContext) {
    let kernel_info = KernelInfoReply::get();
    let reply = ShellReply::Ok(ShellReplyOk::KernelInfo(kernel_info));
    let msg_type = ShellReply::msg_type(&ctx.message.header.msg_type).unwrap();
    let reply = Message {
        zmq_identities: ctx.message.zmq_identities.clone(),
        header: Header::new(msg_type),
        parent_header: Some(ctx.message.header.clone()),
        metadata: Metadata::empty(),
        content: reply,
        buffers: vec![],
    };
    reply.into_multipart().unwrap().send(ctx.socket).unwrap();
}

/// Representation of cell execution in Jupyter.
///
/// Used to keep track of the execution counter and retry attempts for the same
/// cell.
pub struct Cell {
    execution_counter: usize,
    retry_counter: usize,
}

impl Cell {
    /// Construct a new Cell.
    pub const fn new() -> Self {
        Cell {
            execution_counter: 1,
            retry_counter: 1,
        }
    }

    /// Generate a name for the next retry of the current cell.
    ///
    /// This method increases the retry counter each time it is called,
    /// indicating a new attempt on the same cell.
    pub fn next_name(&mut self) -> String {
        let name = format!("cell[{}]#{}", self.execution_counter, self.retry_counter);
        self.retry_counter += 1;
        name
    }

    /// Increment the execution counter after a successful execution.
    ///
    /// Jupyter demands that the execution counter only increases after a
    /// successful execution. This function increments the counter and resets
    /// the retry counter, indicating a new cell execution. It returns the
    /// previous execution counter.
    pub fn success(&mut self) -> usize {
        let current_execution_counter = self.execution_counter;
        self.execution_counter += 1;
        self.retry_counter = 1;
        current_execution_counter
    }
}

fn handle_execute_request(ctx: &mut HandlerContext, request: &ExecuteRequest) {
    let ExecuteRequest {
        code,
        silent,
        store_history,
        user_expressions,
        allow_stdin,
        stop_on_error,
    } = request;
    let msg_type = ShellReply::msg_type(&ctx.message.header.msg_type).unwrap();
    External::apply(ctx.engine_state).unwrap();

    let cell_name = ctx.cell.next_name();
    ctx.konst
        .update(ctx.stack, cell_name.clone(), ctx.message.clone());
    ctx.stdout_handler.update_reply(
        ctx.message.zmq_identities.clone(),
        ctx.message.header.clone(),
    );
    ctx.stderr_handler.update_reply(
        ctx.message.zmq_identities.clone(),
        ctx.message.header.clone(),
    );
    match nu::execute(code, ctx.engine_state, ctx.stack, &cell_name) {
        Ok(data) => handle_execute_results(ctx, msg_type, data),
        Err(error) => handle_execute_error(ctx, msg_type, error),
    };
}

fn handle_execute_error(ctx: &HandlerContext, msg_type: &str, error: ExecuteError) {
    let mut working_set = StateWorkingSet::new(ctx.engine_state);
    let report = match error {
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
        zmq_identities: ctx.message.zmq_identities.clone(),
        header: Header::new(broadcast.msg_type()),
        parent_header: Some(ctx.message.header.clone()),
        metadata: Metadata::empty(),
        content: broadcast,
        buffers: vec![],
    };
    ctx.iopub.send(broadcast.into_multipart().unwrap()).unwrap();

    let reply = ShellReply::Error {
        name: name.to_string(),
        value,
        traceback,
    };
    let reply = Message {
        zmq_identities: ctx.message.zmq_identities.clone(),
        header: Header::new(msg_type),
        parent_header: Some(ctx.message.header.clone()),
        metadata: Metadata::empty(),
        content: reply,
        buffers: vec![],
    };
    reply.into_multipart().unwrap().send(ctx.socket).unwrap();
}

fn handle_execute_results(ctx: &mut HandlerContext, msg_type: &str, pipeline_data: PipelineData) {
    let execution_count = ctx.cell.success();

    if !pipeline_data.is_nothing() {
        let mut render_filter = RENDER_FILTER.lock();
        let render: StringifiedPipelineRender = PipelineRender::render(
            pipeline_data,
            ctx.engine_state,
            ctx.stack,
            ctx.format_decl_ids,
            render_filter.take(),
        )
        .unwrap() // TODO: replace this with some actual handling
        .into();

        let execute_result = ExecuteResult {
            execution_count,
            data: render.data,
            metadata: render.metadata,
        };
        let broadcast = IopubBroacast::from(execute_result);
        let broadcast = Message {
            zmq_identities: ctx.message.zmq_identities.clone(),
            header: Header::new(broadcast.msg_type()),
            parent_header: Some(ctx.message.header.clone()),
            metadata: Metadata::empty(),
            content: broadcast,
            buffers: vec![],
        };
        ctx.iopub.send(broadcast.into_multipart().unwrap()).unwrap();
    }

    let reply = ExecuteReply {
        execution_count,
        user_expressions: json!({}),
    };
    let reply = ShellReply::Ok(ShellReplyOk::Execute(reply));
    let reply = Message {
        zmq_identities: ctx.message.zmq_identities.clone(),
        header: Header::new(msg_type),
        parent_header: Some(ctx.message.header.clone()),
        metadata: Metadata::empty(),
        content: reply,
        buffers: vec![],
    };
    reply.into_multipart().unwrap().send(ctx.socket).unwrap();
}
