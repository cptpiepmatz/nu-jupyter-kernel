use bytes::Bytes;
use nu_protocol::engine::{EngineState, Stack, StateWorkingSet};
use nu_protocol::{FromValue, IntoValue, ShellError, Span, Type, VarId};

use crate::jupyter::messages::{Header, Message};
use crate::CARGO_TOML;

#[derive(Debug, Clone, Copy)]
pub struct Konst {
    var_id: VarId,
}

impl Konst {
    pub const VAR_NAME: &'static str = "nuju";

    pub fn register(engine_state: &mut EngineState) -> Result<Self, ShellError> {
        let mut working_set = StateWorkingSet::new(engine_state);
        let var_id = working_set.add_variable(
            Self::VAR_NAME.as_bytes().to_vec(),
            Span::unknown(),
            Type::Any,
            false,
        );
        engine_state.merge_delta(working_set.render())?;
        Ok(Self { var_id })
    }

    pub fn var_id(&self) -> VarId {
        self.var_id
    }

    pub fn update<C>(&self, stack: &mut Stack, cell_name: String, message: Message<C>) {
        let data = KonstData {
            version: KonstDataVersion {
                kernel: CARGO_TOML.package.version.to_owned(),
                nu: CARGO_TOML
                    .dependencies
                    .nu_engine
                    .version
                    .to_owned(),
            },
            cell: cell_name,
            message: KonstDataMessage {
                zmq_identities: message.zmq_identities,
                header: message.header,
                parent_header: message.parent_header,
            },
        };
        stack.add_var(self.var_id, data.into_value(Span::unknown()))
    }

    pub fn data(&self, stack: &Stack, span: Span) -> Result<KonstData, ShellError> {
        let value = stack
            .get_var(self.var_id, span)
            .map_err(|_| ShellError::VariableNotFoundAtRuntime { span })?;
        KonstData::from_value(value)
    }
}

#[derive(Debug, Clone, IntoValue, FromValue)]
pub struct KonstData {
    pub version: KonstDataVersion,
    pub cell: String,
    pub message: KonstDataMessage,
}

#[derive(Debug, Clone, IntoValue, FromValue)]
pub struct KonstDataVersion {
    pub kernel: String,
    pub nu: String,
}

#[derive(Debug, Clone, IntoValue, FromValue)]
pub struct KonstDataMessage {
    pub zmq_identities: Vec<Bytes>,
    pub header: Header,
    pub parent_header: Option<Header>,
}
