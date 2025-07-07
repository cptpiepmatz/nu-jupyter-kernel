use std::collections::HashMap;

use mime::Mime;
use nu_protocol::PipelineData;
use nu_protocol::engine::{EngineState, Stack, StateWorkingSet};
use parking_lot::Mutex;
use serde_json::json;
use tokio::sync::{broadcast, mpsc};

use super::stream::StreamHandler;
use crate::ShellSocket;
use crate::jupyter::Shutdown;
use crate::jupyter::kernel_info::KernelInfo;
use crate::jupyter::messages::iopub::{self, ExecuteResult, IopubBroacast, Status};
use crate::jupyter::messages::multipart::Multipart;
use crate::jupyter::messages::shell::{
    ExecuteReply, ExecuteRequest, IsCompleteReply, IsCompleteRequest, ShellReply, ShellReplyOk,
    ShellRequest,
};
use crate::jupyter::messages::{Header, Message, Metadata};
use crate::nu::commands::external::External;
use crate::nu::konst::Konst;
use crate::nu::module::KernelInternalSpans;
use crate::nu::render::{FormatDeclIds, PipelineRender, StringifiedPipelineRender};
use crate::nu::{self, ExecuteError, ReportExecuteError};
use crate::util::Select;

// TODO: get rid of this static by passing this into the display command
pub static RENDER_FILTER: Mutex<Option<Mime>> = Mutex::new(Option::None);

pub struct HandlerContext {
    pub socket: ShellSocket,
    pub iopub: mpsc::Sender<Multipart>,
    pub stdout_handler: StreamHandler,
    pub stderr_handler: StreamHandler,
    pub engine_state: EngineState,
    pub format_decl_ids: FormatDeclIds,
    pub konst: Konst,
    pub spans: KernelInternalSpans,
    pub stack: Stack,
    pub cell: Cell,
}

pub async fn handle(mut ctx: HandlerContext, mut shutdown: broadcast::Receiver<Shutdown>) {
    let initial_engine_state = ctx.engine_state.clone();
    let initial_stack = ctx.stack.clone();

    loop {
        let next = tokio::select! {
            biased;
            v = shutdown.recv() => Select::Left(v),
            v = Message::<ShellRequest>::recv(&mut ctx.socket) => Select::Right(v),
        };

        let message = match next {
            Select::Left(Ok(Shutdown { restart: false })) => break,
            Select::Left(Ok(Shutdown { restart: true })) => {
                ctx.engine_state = initial_engine_state.clone();
                ctx.stack = initial_stack.clone();
                // TODO: check if cell counter should get a reset too
                continue;
            }
            Select::Left(Err(_)) => break,
            Select::Right(Ok(msg)) => msg,
            Select::Right(Err(_)) => {
                eprintln!("could not recv message");
                continue;
            }
        };

        send_status(&mut ctx, &message, Status::Busy).await;

        match &message.content {
            ShellRequest::KernelInfo => handle_kernel_info_request(&mut ctx, &message).await,
            ShellRequest::Execute(request) => {
                // take the context out temporarily to allow execution on another thread
                ctx = handle_execute_request(ctx, &message, request).await;
            }
            ShellRequest::IsComplete(request) => {
                handle_is_complete_request(&mut ctx, &message, request).await
            }
        }

        send_status(&mut ctx, &message, Status::Idle).await;
    }
}

async fn send_status(ctx: &mut HandlerContext, message: &Message<ShellRequest>, status: Status) {
    ctx.iopub
        .send(
            status
                .into_message(message.header.clone())
                .into_multipart()
                .unwrap(),
        )
        .await
        .unwrap();
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

async fn handle_kernel_info_request(ctx: &mut HandlerContext, message: &Message<ShellRequest>) {
    let kernel_info = KernelInfo::get();
    let reply = ShellReply::Ok(ShellReplyOk::KernelInfo(kernel_info));
    let msg_type = ShellReply::msg_type(&message.header.msg_type).unwrap();
    let reply = Message {
        zmq_identities: message.zmq_identities.clone(),
        header: Header::new(msg_type),
        parent_header: Some(message.header.clone()),
        metadata: Metadata::empty(),
        content: reply,
        buffers: vec![],
    };
    reply
        .into_multipart()
        .unwrap()
        .send(&mut ctx.socket)
        .await
        .unwrap();
}

async fn handle_execute_request(
    mut ctx: HandlerContext,
    message: &Message<ShellRequest>,
    request: &ExecuteRequest,
) -> HandlerContext {
    let ExecuteRequest {
        code,
        silent,
        store_history,
        user_expressions,
        allow_stdin,
        stop_on_error,
    } = request;
    let msg_type = ShellReply::msg_type(&message.header.msg_type).unwrap();
    External::apply(&mut ctx.engine_state).unwrap();

    let cell_name = ctx.cell.next_name();
    ctx.konst
        .update(&mut ctx.stack, cell_name.clone(), message.clone());
    ctx.stdout_handler
        .update_reply(message.zmq_identities.clone(), message.header.clone());
    ctx.stderr_handler
        .update_reply(message.zmq_identities.clone(), message.header.clone());

    // TODO: place coll in cell, then just pass the cell
    let code = code.to_owned();
    let (executed, mut ctx) = tokio::task::spawn_blocking(move || {
        (
            nu::execute(&code, &mut ctx.engine_state, &mut ctx.stack, &cell_name),
            ctx,
        )
    })
    .await
    .unwrap();
    match executed {
        Ok(data) => handle_execute_results(&mut ctx, message, msg_type, data).await,
        Err(error) => handle_execute_error(&mut ctx, message, msg_type, error).await,
    };

    // reset interrupt signal after every execution, this also notifies the control
    // handler
    ctx.engine_state.reset_signals();

    ctx
}

async fn handle_execute_error(
    ctx: &mut HandlerContext,
    message: &Message<ShellRequest>,
    msg_type: &str,
    error: ExecuteError,
) {
    let mut working_set = StateWorkingSet::new(&ctx.engine_state);
    let (name, value) = {
        // keeping the report makes the following part not Send
        let report = ReportExecuteError::new(error, &mut working_set);
        let name = report.code().to_string();
        let value = report.fmt();
        (name, value)
    };
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
    ctx.iopub
        .send(broadcast.into_multipart().unwrap())
        .await
        .unwrap();

    let reply = ShellReply::Error {
        name,
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
    reply
        .into_multipart()
        .unwrap()
        .send(&mut ctx.socket)
        .await
        .unwrap();
}

async fn handle_execute_results(
    ctx: &mut HandlerContext,
    message: &Message<ShellRequest>,
    msg_type: &str,
    pipeline_data: PipelineData,
) {
    let execution_count = ctx.cell.success();

    if !pipeline_data.is_nothing() {
        let render: StringifiedPipelineRender = {
            // render filter needs to be dropped until next async yield
            let mut render_filter = RENDER_FILTER.lock();
            PipelineRender::render(
                pipeline_data,
                &ctx.engine_state,
                &mut ctx.stack,
                &ctx.spans,
                ctx.format_decl_ids,
                render_filter.take(),
            )
            .unwrap() // TODO: replace this with some actual handling
            .into()
        };

        let execute_result = ExecuteResult {
            execution_count,
            data: render.data,
            metadata: render.metadata,
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
        ctx.iopub
            .send(broadcast.into_multipart().unwrap())
            .await
            .unwrap();
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
    reply
        .into_multipart()
        .unwrap()
        .send(&mut ctx.socket)
        .await
        .unwrap();
}

async fn handle_is_complete_request(
    ctx: &mut HandlerContext,
    message: &Message<ShellRequest>,
    request: &IsCompleteRequest,
) {
    let reply = IsCompleteReply::Unknown;
    let reply = ShellReply::Ok(ShellReplyOk::IsComplete(reply));
    let msg_type = ShellReply::msg_type(&message.header.msg_type).unwrap();
    let reply = Message {
        zmq_identities: message.zmq_identities.clone(),
        header: Header::new(msg_type),
        parent_header: Some(message.header.clone()),
        metadata: Metadata::empty(),
        content: reply,
        buffers: vec![],
    };
    reply
        .into_multipart()
        .unwrap()
        .send(&mut ctx.socket)
        .await
        .unwrap();
}
