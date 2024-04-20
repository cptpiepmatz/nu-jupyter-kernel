use nu_protocol::debugger::WithoutDebug;
use nu_protocol::engine::{EngineState, Stack, StateWorkingSet};
use nu_protocol::PipelineData;

static INPUT: &str = "{a: 3} | to json";

fn get_engine_state() -> EngineState {
    // TODO: compare with nu_cli::get_engine_state for other contexts
    let engine_state = nu_cmd_lang::create_default_context();
    let engine_state = nu_command::add_shell_command_context(engine_state);
    engine_state
}

fn execute_some_input() {
    let input = INPUT.as_bytes();
    let mut engine_state = get_engine_state();
    let mut working_set = StateWorkingSet::new(&engine_state);
    let block = nu_parser::parse(&mut working_set, None, input, false);

    if let Some(error) = working_set.parse_errors.first() {
        println!("got some error: {error}");
        return;
    }

    engine_state
        .merge_delta(working_set.delta)
        .expect("could not merge delta");
    let mut stack = Stack::new();
    let res = nu_engine::eval_block::<WithoutDebug>(
        &engine_state,
        &mut stack,
        &block,
        PipelineData::empty(),
    );
    dbg!(res);
}
