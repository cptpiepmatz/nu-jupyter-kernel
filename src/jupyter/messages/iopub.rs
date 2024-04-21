use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum IopubBroacast {
    Stream,
    DisplayData,
    UpdateDisplayData,
    ExecuteInput,
    ExecuteResult,
    Error,
    Status(Status),
    ClearOutput,
    DebugEvent,
}

impl IopubBroacast {
    pub fn msg_type(&self) -> &'static str {
        match self {
            IopubBroacast::Stream => "stream",
            IopubBroacast::DisplayData => "display_data",
            IopubBroacast::UpdateDisplayData => "update_display_data",
            IopubBroacast::ExecuteInput => "execute_input",
            IopubBroacast::ExecuteResult => "execute_result",
            IopubBroacast::Error => "error",
            IopubBroacast::Status(_) => "status",
            IopubBroacast::ClearOutput => "clear_output",
            IopubBroacast::DebugEvent => "debug_event",
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "execution_state", rename_all = "snake_case")]
pub enum Status {
    Busy,
    Idle,
    Starting,
}
