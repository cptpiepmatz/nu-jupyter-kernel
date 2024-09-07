use serde::{Deserialize, Serialize};

use crate::jupyter::kernel_info::KernelInfo;
use crate::jupyter::Shutdown;

#[derive(Debug, Deserialize, Clone)]
pub enum ControlRequest {
    KernelInfo,
    Shutdown(Shutdown),
    Interrupt,
    Debug(DebugRequest),
}

impl ControlRequest {
    pub fn parse_variant(variant: &str, body: &str) -> Result<Self, ()> {
        match variant {
            "kernel_info_request" => Ok(Self::KernelInfo),
            "shutdown_request" => Ok(Self::Shutdown(serde_json::from_str(body).unwrap())),
            "interrupt_request" => Ok(Self::Interrupt),
            "debug_request" => Ok(Self::Debug(serde_json::from_str(body).unwrap())),
            _ => {
                eprintln!("found it here: {variant}");

                eprintln!("unknown request {variant}");
                Err(())
            }
        }
    }
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum ControlReply {
    Ok(ControlReplyOk),
    Error {
        #[serde(rename = "ename")]
        name: String,
        #[serde(rename = "evalue")]
        value: String,
        traceback: Vec<String>,
    },
}

impl ControlReply {
    pub fn msg_type(request_type: &'_ str) -> Result<&'static str, ()> {
        Ok(match request_type {
            "kernel_info_request" => "kernel_info_reply",
            "shutdown_request" => "shutdown_reply",
            "interrupt_request" => "interrupt_reply",
            "debug_request" => "debug_reply",
            _ => todo!("handle unknown requests"),
        })
    }
}

#[derive(Debug, Serialize, Clone)]
#[serde(untagged)]
pub enum ControlReplyOk {
    KernelInfo(KernelInfo),
    Shutdown(Shutdown),
    Interrupt,
    Debug(DebugReply),
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type", rename = "request")]
pub struct DebugRequest {
    pub command: DebugRequestCommand
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum DebugRequestCommand {
    DumpCell,
    DebugInfo,
    InspectVariables,
    RichInspectVariables,
    CopyToGlobals,
}

#[derive(Debug, Serialize, Clone)]
#[serde(tag = "type", rename = "response")]
pub struct DebugReply {
    pub success: bool,
    pub body: DebugReplyBody,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum DebugReplyBody {
    DumpCell,
    DebugInfo {
        is_started: bool,
        hash_method: String,
        hash_seed: String,
        tmp_file_prefix: String,
        tmp_file_suffix: String,
        breakpoints: Vec<DebugBreakpoint>,
        stopped_threads: Vec<usize>,
        rich_rendering: bool,
        exception_paths: Vec<String>,
    },
    InspectVariables,
    RichInspectVariables,
    CopyToGlobals,
}

#[derive(Debug, Serialize, Clone)]
pub struct DebugBreakpoint {
    pub source: String,
    pub breakpoints: Vec<SourceBreakpoint>,
}

// spec: https://microsoft.github.io/debug-adapter-protocol/specification#Types_SourceBreakpoint
#[derive(Debug, Serialize, Clone)]
pub struct SourceBreakpoint {
    
}