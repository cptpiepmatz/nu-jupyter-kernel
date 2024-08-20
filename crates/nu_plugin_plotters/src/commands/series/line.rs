use nu_engine::command_prelude::*;
use nu_protocol::LabeledError;

use crate::value::series_2d::Series2d;

#[derive(Debug, Clone)]
pub struct LineSeries;

impl Command for LineSeries {
    fn name(&self) -> &str {
        "series line"
    }

    fn signature(&self) -> Signature {
        Signature::new(self.name())
            .usage(self.usage())
            .search_terms(
                self.search_terms()
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
                Series2d::ty(),
            )
    }

    fn usage(&self) -> &str {
        todo!()
    }

    fn search_terms(&self) -> Vec<&str> {
        vec!["plotters", "series", "line", "chart"]
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

impl nu_plugin::SimplePluginCommand for LineSeries {
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
        engine: &nu_plugin::EngineInterface,
        call: &nu_plugin::EvaluatedCall,
        input: &Value,
    ) -> Result<Value, LabeledError> {
        todo!()
    }
}

impl LineSeries {}
