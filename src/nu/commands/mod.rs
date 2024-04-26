use std::fmt::Write;

use nu_protocol::engine::{EngineState, Stack, StateWorkingSet};
use nu_protocol::Category;

pub mod external;

const COMMAND_GROUP: &str = "nujy";

// TODO: check out which more should be hidden
static INCOMPATIBLE_COMMANDS: &[&str] = &["input", "exit", "run-external"];

/// Hide incompatible commands so that users don't accidentally call them.
pub fn hide_incompatible_commands(
    engine_state: &mut EngineState,
) -> Result<(), super::ExecuteError> {
    let mut code = String::new();
    for command in INCOMPATIBLE_COMMANDS {
        writeln!(code, "hide {command}").expect("String::write is infallible");
    }

    let mut stack = Stack::new();
    super::execute(&code, engine_state, &mut stack)?;
    Ok(())
}

pub fn category() -> Category {
    Category::Custom("jupyter".to_owned())
}

pub fn add_jupyter_command_context(mut engine_state: EngineState) -> EngineState {
    let delta = {
        let mut working_set = StateWorkingSet::new(&engine_state);

        macro_rules! bind_command {
            ( $( $command:expr ),* $(,)? ) => {
                $( working_set.add_decl(Box::new($command)); )*
            };
        }

        bind_command! {
            external::External
        }

        working_set.render()
    };

    if let Err(err) = engine_state.merge_delta(delta) {
        eprintln!("Error creating jupyter context: {err:?}");
    }

    engine_state
}
