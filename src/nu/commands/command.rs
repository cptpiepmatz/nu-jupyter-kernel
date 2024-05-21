use nu_engine::command_prelude::*;
use nu_engine::get_full_help;

use super::COMMANDS_TOML;

#[derive(Clone)]
pub struct Nuju;

impl Command for Nuju {
    fn name(&self) -> &str {
        COMMANDS_TOML.nuju.name
    }

    fn usage(&self) -> &str {
        COMMANDS_TOML.nuju.usage
    }

    fn signature(&self) -> nu_protocol::Signature {
        Signature::build(Self.name())
            .category(super::category())
            .input_output_types(vec![(Type::Nothing, Type::String)])
    }

    fn extra_usage(&self) -> &str {
        COMMANDS_TOML.nuju.extra_usage
    }

    fn run(
        &self,
        engine_state: &EngineState,
        stack: &mut Stack,
        call: &Call,
        _input: PipelineData,
    ) -> Result<PipelineData, ShellError> {
        Ok(
            Value::string(get_full_help(&Nuju, engine_state, stack), call.head)
                .into_pipeline_data(),
        )
    }
}
