use nu_protocol::engine::{EngineState, Stack, StateWorkingSet};
use nu_protocol::{record, ShellError, Span, Type, Value, VarId};

use crate::jupyter::messages::Message;
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

    pub fn update<C>(&self, stack: &mut Stack, cell_name: &str, message: Message<C>) {
        // the `test_*` method all set the span to unknown which I would do
        // anyways here
        let value = Value::test_record(record! {
            "version" => Value::test_record(record! {
                "kernel" => Value::test_string(CARGO_TOML.package.version),
                "nu" => Value::test_string(CARGO_TOML.dependencies.nu_engine.version),
            }),
            "cell" => Value::test_string(cell_name),
            "message" => message.into()
        });
        stack.add_var(self.var_id, value)
    }
}

// TODO: make this represent the value of the constant
pub struct KonstData;

impl<C> From<Message<C>> for Value {
    fn from(value: Message<C>) -> Self {
        Value::test_record(record! {
            "zmq_identities" => Value::test_list(value
                .zmq_identities
                .into_iter()
                .map(|zmq_id| Value::test_binary(zmq_id))
                .collect()
            ),
            "header" => Value::test_record(record! {
                "msg_id" => Value::test_string(value.header.msg_id),
                "session" => Value::test_string(value.header.session),
                "username" => Value::test_string(value.header.username),
                "date" => Value::test_string(value.header.date),
                "msg_type" => Value::test_string(value.header.msg_type),
                "version" => Value::test_string(value.header.version),
            }),
            "parent_header" => match value.parent_header {
                None => Value::test_nothing(),
                Some(ph) => Value::test_record(record! {
                    "msg_id" => Value::test_string(ph.msg_id),
                    "session" => Value::test_string(ph.session),
                    "username" => Value::test_string(ph.username),
                    "date" => Value::test_string(ph.date),
                    "msg_type" => Value::test_string(ph.msg_type),
                    "version" => Value::test_string(ph.version),
                }),
            },
        })
    }
}
