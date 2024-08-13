use bytes::Bytes;
use nu_protocol::engine::{EngineState, Stack, StateWorkingSet};
use nu_protocol::{record, Record, ShellError, Span, Type, Value, VarId};

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
                nu: CARGO_TOML.dependencies.nu_engine.version.to_owned(),
            },
            cell: cell_name,
            message: KonstDataMessage {
                zmq_identities: message.zmq_identities,
                header: message.header,
                parent_header: message.parent_header,
            },
        };
        stack.add_var(self.var_id, data.into())
    }

    pub fn data(&self, stack: &Stack, span: Span) -> Result<KonstData, ShellError> {
        let value = stack
            .get_var(self.var_id, span)
            .map_err(|_| KonstIntegrityError::MissingField {
                path: "$nuju".to_owned(),
                expected: KonstData::ty(),
            })
            .map_err(|err| err.into_shell_error(span))?;
        KonstData::try_from(value).map_err(|err| err.into_shell_error(span))
    }
}

#[derive(Debug)]
pub enum KonstIntegrityError {
    MissingField {
        path: String,
        expected: Type,
    },
    IncorrectType {
        path: String,
        unexpected: Type,
        expected: Type,
    },
}

impl KonstIntegrityError {
    pub fn into_shell_error(self, span: Span) -> ShellError {
        let help = Some(
            "This error is unexpected.\nPlease consider opening a ticket at\nhttps://github.com/cptpiepmatz/nu-jupyter-kernel/issues"
            .to_owned()
        );
        let nuju = "$nuju"; // makes the format! easier
        match self {
            KonstIntegrityError::MissingField { path, expected } => ShellError::GenericError {
                error: "$nuju field missing".to_owned(),
                msg: format!(
                    "Catastrophic error: the expected path {path:?} is missing in the constant \
                     for {nuju:?}.\nExpected type: {expected}"
                ),
                span: Some(span),
                help,
                inner: vec![],
            },
            KonstIntegrityError::IncorrectType {
                path,
                unexpected,
                expected,
            } => ShellError::GenericError {
                error: "$nuju field has incorrect type".to_owned(),
                msg: format!(
                    "Catastrophic error: the path {path:?} has type {unexpected}, but {expected} \
                     was expected."
                ),
                span: Some(span),
                help,
                inner: vec![],
            },
        }
    }
}

macro_rules! match_value {
    ($value:expr, $path:expr, $variant:path, $expected:expr) => {
        match $value {
            $variant { val, .. } => Ok(val),
            value => {
                let ty = value.get_type();
                Err(KonstIntegrityError::IncorrectType {
                    path: $path.into(),
                    unexpected: ty,
                    expected: $expected,
                })
            }
        }
    };
}

macro_rules! extract_field {
    ($record:expr, $field:literal, $path:expr, $variant:path, $expected:expr) => {{
        let val = $record
            .remove($field)
            .ok_or_else(|| KonstIntegrityError::MissingField {
                path: $path.into(),
                expected: $expected,
            })?;
        match_value!(val, $path, $variant, $expected)
    }};
}

macro_rules! record_type {
    ($($field:ident : $type:expr),* $(,)?) => {
        Type::Record(
            vec![
                $( (stringify!($field).to_owned(), $type), )*
            ]
            .into_boxed_slice()
        )
    };
}

#[derive(Debug, Clone)]
pub struct KonstData {
    pub version: KonstDataVersion,
    pub cell: String,
    pub message: KonstDataMessage,
}

impl From<KonstData> for Value {
    fn from(data: KonstData) -> Self {
        Value::test_record(record! {
            "version" => data.version.into(),
            "cell" => Value::test_string(data.cell),
            "message" => data.message.into()
        })
    }
}

impl KonstData {
    fn ty() -> Type {
        record_type! {
            version: KonstDataVersion::ty(),
            cell: Type::String,
            message: KonstDataMessage::ty()
        }
    }
}

impl TryFrom<Value> for KonstData {
    type Error = KonstIntegrityError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        let mut data: Record =
            match_value!(value, "$nuju", Value::Record, Self::ty())?.into_owned();

        let version = data
            .remove("version")
            .ok_or_else(|| KonstIntegrityError::MissingField {
                path: "$nuju.version".to_owned(),
                expected: KonstDataVersion::ty(),
            })?
            .try_into()?;

        let cell = extract_field!(data, "cell", "$nuju.cell", Value::String, Type::String)?;

        let message = data
            .remove("message")
            .ok_or_else(|| KonstIntegrityError::MissingField {
                path: "$nuju.message".to_owned(),
                expected: KonstDataMessage::ty(),
            })?
            .try_into()?;

        Ok(Self {
            version,
            cell,
            message,
        })
    }
}

#[derive(Debug, Clone)]
pub struct KonstDataVersion {
    pub kernel: String,
    pub nu: String,
}

impl From<KonstDataVersion> for Value {
    fn from(data: KonstDataVersion) -> Self {
        Value::test_record(record! {
            "kernel" => Value::test_string(data.kernel),
            "nu" => Value::test_string(data.nu),
        })
    }
}

impl KonstDataVersion {
    fn ty() -> Type {
        record_type! {
            kernel: Type::String,
            nu: Type::String
        }
    }
}

impl TryFrom<Value> for KonstDataVersion {
    type Error = KonstIntegrityError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        let mut version: Record =
            match_value!(value, "$nuju.version", Value::Record, Self::ty())?.into_owned();

        let kernel = extract_field!(
            version,
            "kernel",
            "$nuju.version.kernel",
            Value::String,
            Type::String
        )?;
        let nu = extract_field!(
            version,
            "nu",
            "$nuju.version.nu",
            Value::String,
            Type::String
        )?;

        Ok(Self { kernel, nu })
    }
}

#[derive(Debug, Clone)]
pub struct KonstDataMessage {
    pub zmq_identities: Vec<Bytes>,
    pub header: Header,
    pub parent_header: Option<Header>,
}

impl From<KonstDataMessage> for Value {
    fn from(data: KonstDataMessage) -> Self {
        Value::test_record(record! {
            "zmq_identities" => Value::test_list(data
                .zmq_identities
                .into_iter()
                .map(Value::test_binary)
                .collect()
            ),
            "header" => Value::test_record(record! {
                "msg_id" => Value::test_string(data.header.msg_id),
                "session" => Value::test_string(data.header.session),
                "username" => Value::test_string(data.header.username),
                "date" => Value::test_string(data.header.date),
                "msg_type" => Value::test_string(data.header.msg_type),
                "version" => Value::test_string(data.header.version),
            }),
            "parent_header" => match data.parent_header {
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

impl KonstDataMessage {
    fn ty() -> Type {
        record_type! {
            zmq_identities: Self::zmq_ids_ty(),
            header: Self::header_ty(),
            parent_header: Type::Any, // we have no optional type
        }
    }

    fn zmq_ids_ty() -> Type {
        Type::List(Box::new(Type::Binary))
    }

    fn header_ty() -> Type {
        record_type! {
            msg_id: Type::String,
            session: Type::String,
            username: Type::String,
            date: Type::String,
            msg_type: Type::String,
            version: Type::String,
        }
    }
}

impl TryFrom<Value> for KonstDataMessage {
    type Error = KonstIntegrityError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        let mut message: Record =
            match_value!(value, "$nuju.message", Value::Record, Self::ty())?.into_owned();

        let zmq_identities = extract_zmq_identities(&mut message)?;
        let header = extract_header(&mut message)?;
        let parent_header = extract_parent_header(&mut message)?;

        Ok(KonstDataMessage {
            zmq_identities,
            header,
            parent_header,
        })
    }
}

fn extract_zmq_identities(message: &mut Record) -> Result<Vec<Bytes>, KonstIntegrityError> {
    // Value::List doesn't have `val`, it has `vals`
    let zmq_identities =
        message
            .remove("zmq_identities")
            .ok_or_else(|| KonstIntegrityError::MissingField {
                path: "$nuju.message.zmq_identities".to_owned(),
                expected: KonstDataMessage::zmq_ids_ty(),
            })?;
    let zmq_identities = match zmq_identities {
        Value::List { vals, .. } => vals,
        value => {
            let ty = value.get_type();
            return Err(KonstIntegrityError::IncorrectType {
                path: "$nuju.message.zmq_identities".to_owned(),
                unexpected: ty,
                expected: KonstDataMessage::zmq_ids_ty(),
            });
        }
    };
    let zmq_identities = zmq_identities
        .into_iter()
        .enumerate()
        .map(|(i, val)| {
            match_value!(
                val,
                format!("$nuju.message.zmq_identities.{}", i),
                Value::Binary,
                Type::Binary
            )
            .map(Bytes::from)
        })
        .collect::<Result<Vec<Bytes>, KonstIntegrityError>>()?;
    Ok(zmq_identities)
}

fn extract_header(message: &mut Record) -> Result<Header, KonstIntegrityError> {
    let mut header = extract_field!(
        message,
        "header",
        "$nuju.message.header",
        Value::Record,
        KonstDataMessage::header_ty()
    )?
    .into_owned();
    let msg_id = extract_field!(
        header,
        "msg_id",
        "$nuju.message.header.msg_id",
        Value::String,
        Type::String
    )?;
    let session = extract_field!(
        header,
        "session",
        "$nuju.message.header.session",
        Value::String,
        Type::String
    )?;
    let username = extract_field!(
        header,
        "username",
        "$nuju.message.header.username",
        Value::String,
        Type::String
    )?;
    let date = extract_field!(
        header,
        "date",
        "$nuju.message.header.date",
        Value::String,
        Type::String
    )?;
    let msg_type = extract_field!(
        header,
        "msg_type",
        "$nuju.message.header.msg_type",
        Value::String,
        Type::String
    )?;
    let version = extract_field!(
        header,
        "version",
        "$nuju.message.header.version",
        Value::String,
        Type::String
    )?;
    let header = Header {
        msg_id,
        session,
        username,
        date,
        msg_type,
        version,
    };
    Ok(header)
}

fn extract_parent_header(message: &mut Record) -> Result<Option<Header>, KonstIntegrityError> {
    let parent_header = match message.remove("parent_header") {
        None => None,
        Some(Value::Nothing { .. }) => None,
        Some(Value::Record { val, .. }) => Some(val.into_owned()),
        Some(value) => {
            let ty = value.get_type();
            return Err(KonstIntegrityError::IncorrectType {
                path: "$nuju.message.parent_header".to_owned(),
                unexpected: ty,
                expected: KonstDataMessage::header_ty(),
            });
        }
    };
    let parent_header = parent_header
        .map(|mut header| {
            let msg_id = extract_field!(
                header,
                "msg_id",
                "$nuju.message.header.msg_id",
                Value::String,
                Type::String
            )?;
            let session = extract_field!(
                header,
                "session",
                "$nuju.message.header.session",
                Value::String,
                Type::String
            )?;
            let username = extract_field!(
                header,
                "username",
                "$nuju.message.header.username",
                Value::String,
                Type::String
            )?;
            let date = extract_field!(
                header,
                "date",
                "$nuju.message.header.date",
                Value::String,
                Type::String
            )?;
            let msg_type = extract_field!(
                header,
                "msg_type",
                "$nuju.message.header.msg_type",
                Value::String,
                Type::String
            )?;
            let version = extract_field!(
                header,
                "version",
                "$nuju.message.header.version",
                Value::String,
                Type::String
            )?;
            let header = Header {
                msg_id,
                session,
                username,
                date,
                msg_type,
                version,
            };
            Ok(header)
        })
        .transpose()?;
    Ok(parent_header)
}
