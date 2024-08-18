mod chart_2d;
mod chart_builder;

pub use chart_builder::ChartBuilderOptions;

macro_rules! impl_custom_value {
    ($type:ident) => {
        #[typetag::serde]
        impl nu_protocol::CustomValue for $type {
            fn clone_value(&self, span: nu_protocol::Span) -> nu_protocol::Value {
                nu_protocol::Value::custom(Box::new(self.clone()), span)
            }

            fn type_name(&self) -> String {
                Self::expected_type().to_string()
            }

            fn to_base_value(
                &self,
                span: nu_protocol::Span,
            ) -> Result<nu_protocol::Value, nu_protocol::ShellError> {
                Ok(self.clone().into_value(span))
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            fn as_mut_any(&mut self) -> &mut dyn std::any::Any {
                self
            }
        }
    };
}

pub(crate) use impl_custom_value;
