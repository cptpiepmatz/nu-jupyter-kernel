use nu_engine::{command_prelude::*, get_full_help};

#[derive(Clone)]
pub struct Nuju;

impl Command for Nuju {
    fn name(&self) -> &str {
        super::COMMAND_GROUP
    }

    fn usage(&self) -> &str {
        "Control behavior of the kernel."
    }

    fn signature(&self) -> nu_protocol::Signature {
        Signature::build(Self.name())
            .category(super::category())
            .input_output_types(vec![(Type::Nothing, Type::String)])
    }

    fn extra_usage(&self) -> &str {
        "You must use one of the following subcommands.\n\
        Using this command as-is will only produce this help message."
    }

    fn run(
        &self,
        engine_state: &EngineState,
        stack: &mut Stack,
        call: &Call,
        _input: PipelineData,
    ) -> Result<PipelineData, ShellError> {
        Ok(Value::string(
            get_full_help(
                &Nuju.signature(),
                &Nuju.examples(),
                engine_state,
                stack,
                self.is_parser_keyword(),
            ),
            call.head,
        )
        .into_pipeline_data())
    }
}
