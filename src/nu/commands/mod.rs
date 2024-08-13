use std::fmt::Write;
use std::sync::mpsc;

use nu_protocol::engine::{EngineState, Stack, StateWorkingSet};
use nu_protocol::Category;

use super::konst::Konst;
use super::render::FormatDeclIds;
use crate::jupyter::messages::multipart::Multipart;

pub mod command;
pub mod display;
pub mod external;
pub mod print;

static_toml::static_toml! {
    const COMMANDS_TOML = include_toml!("commands.toml");
}

/// Hide incompatible commands so that users don't accidentally call them.
pub fn hide_incompatible_commands(
    engine_state: &mut EngineState,
) -> Result<(), super::ExecuteError> {
    let mut code = String::new();
    for command in COMMANDS_TOML.incompatible_commands {
        writeln!(code, "hide {command}").expect("String::write is infallible");
    }

    let mut stack = Stack::new();
    super::execute(&code, engine_state, &mut stack, "hide-initial-commands")?;
    Ok(())
}

pub fn category() -> Category {
    Category::Custom("jupyter".to_owned())
}

pub struct JupyterCommandContext {
    pub iopub: mpsc::Sender<Multipart>,
    pub format_decl_ids: FormatDeclIds,
    pub konst: Konst,
}

pub fn add_jupyter_command_context(
    mut engine_state: EngineState,
    ctx: JupyterCommandContext,
) -> EngineState {
    let delta = {
        let mut working_set = StateWorkingSet::new(&engine_state);

        macro_rules! bind_command {
            ( $( $command:expr ),* $(,)? ) => {
                $( working_set.add_decl(Box::new($command)); )*
            };
        }

        bind_command! {
            command::Nuju,
            external::External,
            display::Display,
            print::Print::new(ctx.iopub, ctx.format_decl_ids, ctx.konst)
        }

        working_set.render()
    };

    if let Err(err) = engine_state.merge_delta(delta) {
        eprintln!("Error creating jupyter context: {err:?}");
    }

    engine_state
}
