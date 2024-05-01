use std::sync::atomic::{AtomicBool, Ordering};

use atomic_enum::atomic_enum;
use const_format::formatcp;
use nu_protocol::ast::Call;
use nu_protocol::engine::{Command, EngineState, Stack, StateWorkingSet};
use nu_protocol::{Category, PipelineData, ShellError, Signature, Span, Type, Value};

use super::COMMANDS_TOML;

#[atomic_enum]
#[derive(PartialEq)]
enum ExternalState {
    Disabled = 0,
    JustEnabled,
    AlreadyEnabled,
}

impl AtomicExternalState {
    pub fn fetch_max(&self, val: ExternalState, order: Ordering) -> ExternalState {
        match self.0.fetch_max(val as usize, order) {
            0 => ExternalState::Disabled,
            1 => ExternalState::JustEnabled,
            2 => ExternalState::AlreadyEnabled,
            _ => unreachable!("ExternalState represents at max 2"),
        }
    }
}

static EXTERNAL_STATE: AtomicExternalState = AtomicExternalState::new(ExternalState::Disabled);

#[derive(Debug, Clone)]
pub struct External;

impl Command for External {
    fn name(&self) -> &str {
        COMMANDS_TOML.external.name
    }

    fn usage(&self) -> &str {
        COMMANDS_TOML.external.usage
    }

    fn extra_usage(&self) -> &str {
        COMMANDS_TOML.external.extra_usage
    }

    fn search_terms(&self) -> Vec<&str> {
        COMMANDS_TOML.external.search_tems.into()
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .input_output_types(vec![(Type::Any, Type::Nothing)])
            .category(super::category())
    }

    fn run(
        &self,
        _engine_state: &EngineState,
        _stack: &mut Stack,
        _call: &Call,
        _input: PipelineData,
    ) -> Result<PipelineData, ShellError> {
        // TODO: add some display data iopub here
        // update the value to at least `JustEnabled`
        EXTERNAL_STATE.fetch_max(ExternalState::JustEnabled, Ordering::SeqCst);
        Ok(PipelineData::Value(Value::nothing(Span::unknown()), None))
    }
}

impl External {
    /// Apply the `run-external` command to the engine if external commands were
    /// just enabled.
    pub fn apply(engine_state: &mut EngineState) -> Result<(), ShellError> {
        if let ExternalState::JustEnabled = EXTERNAL_STATE.load(Ordering::SeqCst) {
            let mut working_set = StateWorkingSet::new(&engine_state);
            // TODO: add a command that controls the output of external calls
            working_set.add_decl(Box::new(nu_command::External));
            engine_state.merge_delta(working_set.render())?;
            EXTERNAL_STATE.swap(ExternalState::AlreadyEnabled, Ordering::SeqCst);
        }
        Ok(())
    }
}
