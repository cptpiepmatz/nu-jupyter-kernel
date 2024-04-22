use std::collections::HashMap;

use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum IopubBroacast {
    Stream,
    DisplayData(DisplayData),
    UpdateDisplayData,
    ExecuteInput,
    ExecuteResult(ExecuteResult),
    Error,
    Status(Status),
    ClearOutput,
    DebugEvent,
}

impl IopubBroacast {
    pub fn msg_type(&self) -> &'static str {
        match self {
            IopubBroacast::Stream => "stream",
            IopubBroacast::DisplayData(_) => "display_data",
            IopubBroacast::UpdateDisplayData => "update_display_data",
            IopubBroacast::ExecuteInput => "execute_input",
            IopubBroacast::ExecuteResult(_) => "execute_result",
            IopubBroacast::Error => "error",
            IopubBroacast::Status(_) => "status",
            IopubBroacast::ClearOutput => "clear_output",
            IopubBroacast::DebugEvent => "debug_event",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct DisplayData {
    pub data: HashMap<String, String>,
    pub metadata: HashMap<String, String>,
    pub transient: HashMap<String, String>,
}

#[derive(Debug, Serialize)]
pub struct ExecuteResult {
    pub execution_count: usize,
    pub data: HashMap<String, String>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "execution_state", rename_all = "snake_case")]
pub enum Status {
    Busy,
    Idle,
    Starting,
}
