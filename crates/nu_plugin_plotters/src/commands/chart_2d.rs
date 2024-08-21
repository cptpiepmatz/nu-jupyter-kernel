use nu_engine::command_prelude::*;
use nu_plugin::{EngineInterface, EvaluatedCall, SimplePluginCommand};
use nu_protocol::LabeledError;

use crate::value;

#[derive(Debug, Clone)]
pub struct Chart2d;

impl Command for Chart2d {
    fn name(&self) -> &str {
        "chart 2d"
    }

    fn signature(&self) -> Signature {
        Signature::new(Command::name(self))
            .add_help()
            .usage(Command::usage(self))
            .extra_usage(Command::extra_usage(self))
            .search_terms(
                Command::search_terms(self)
                    .into_iter()
                    .map(ToOwned::to_owned)
                    .collect(),
            )
            .named(
                "color",
                crate::value::color::Color::syntax_shape(),
                "Define the color of the points and the line. For valid color inputs, refer to \
                 `plotters colors --help`.",
                Some('c'),
            )
            .named(
                "filled",
                SyntaxShape::Boolean,
                "Define whether the points in the series should be filled.",
                Some('f'),
            )
            .named(
                "stroke-width",
                SyntaxShape::Int,
                "Define the width of the stroke.",
                Some('s'),
            )
            .named(
                "point-size",
                SyntaxShape::Int,
                "Define the size of the points in pixels.",
                Some('p'),
            )
            .input_output_type(Type::list(Type::Number), Series2d::ty())
            .input_output_type(Type::list(Type::list(Type::Number)), Series2d::ty())
            .input_output_type(
                Type::list(Type::Record(
                    vec![
                        ("x".to_string(), Type::Number),
                        ("y".to_string(), Type::Number),
                    ]
                    .into_boxed_slice(),
                )),
                value::chart_2d::Chart2d::ty(),
            )
    }

    fn usage(&self) -> &str {
        todo!()
    }

    fn search_terms(&self) -> Vec<&str> {
        vec!["plotters", "chart", "2d"]
    }

    fn run(
        &self,
        engine_state: &EngineState,
        stack: &mut Stack,
        call: &Call,
        input: PipelineData,
    ) -> Result<PipelineData, ShellError> {
        todo!()
    }
}

impl SimplePluginCommand for Chart2d {
    type Plugin = crate::plugin::PlottersPlugin;

    fn name(&self) -> &str {
        Command::name(self)
    }

    fn signature(&self) -> Signature {
        Command::signature(self)
    }

    fn usage(&self) -> &str {
        Command::usage(self)
    }

    fn search_terms(&self) -> Vec<&str> {
        Command::search_terms(self)
    }

    fn run(
        &self,
        plugin: &Self::Plugin,
        engine: &EngineInterface,
        call: &EvaluatedCall,
        input: &Value,
    ) -> Result<Value, LabeledError> {
        todo!()
    }
}

impl Chart2d {}
