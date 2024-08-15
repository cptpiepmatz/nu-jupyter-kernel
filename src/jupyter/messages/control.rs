use serde::{Deserialize, Serialize};

use crate::jupyter::kernel_info::KernelInfo;
use crate::jupyter::Shutdown;

#[derive(Debug, Deserialize, Clone)]
pub enum ControlRequest {
    KernelInfo,
    Shutdown(Shutdown),
    Interrupt, // TODO: add these
    Debug,
}

impl ControlRequest {
    pub fn parse_variant(variant: &str, body: &str) -> Result<Self, ()> {
        match variant {
            "kernel_info_request" => Ok(Self::KernelInfo),
            "shutdown_request" => Ok(Self::Shutdown(serde_json::from_str(body).unwrap())),
            "interrupt_request" => todo!(),
            "debug_request" => todo!(),
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
    Debug,
}
