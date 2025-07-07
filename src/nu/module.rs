use indoc::indoc;
use nu_protocol::ast::{Argument, Block, Expr};
use nu_protocol::engine::{EngineState, StateWorkingSet};
use nu_protocol::{BlockId, DeclId, Span};

const MODULE: &str = indoc! {r#"
    module nuju {
        module render {
            @kernel-internal
            def "text" []: any -> string {...}

            @kernel-internal
            def "csv" []: any -> string {...}

            @kernel-internal
            def "json" []: any -> string {...}

            @kernel-internal
            def "html" []: any -> string {...}

            @kernel-internal
            def "md" []: any -> string {...}

            @kernel-internal
            def "svg" []: any -> string {...}
        }
    }
"#};

#[derive(Debug, Clone)]
pub struct KernelInternalSpans {
    pub render: KernelInternalRenderSpans,
}

#[derive(Debug, Clone)]
pub struct KernelInternalRenderSpans {
    pub text: Span,
    pub csv: Span,
    pub json: Span,
    pub html: Span,
    pub md: Span,
    pub svg: Span,
}

pub fn create_nuju_module(engine_state: &mut EngineState) -> KernelInternalSpans {
    let mut working_set = StateWorkingSet::new(engine_state);

    let file_id = working_set.add_file("nuju internals".to_string(), MODULE.as_bytes());
    let file_span = working_set.get_span_for_file(file_id);

    let (outer_block, ..) = nu_parser::parse_module_block(&mut working_set, file_span, b"nuju");
    let nuju_block_id = find_inner_block(&outer_block, "nuju").expect("find nuju block");
    let nuju_block = working_set.get_block(nuju_block_id);
    let render_block_id = find_inner_block(nuju_block, "render").expect("find nuju/render block");
    let render_block = working_set.get_block(render_block_id);

    let (_, render_text_span) = find_decl(render_block, "text").expect("find render/text decl");
    let (_, render_csv_span) = find_decl(render_block, "csv").expect("find render/csv decl");
    let (_, render_json_span) = find_decl(render_block, "json").expect("find render/json decl");
    let (_, render_html_span) = find_decl(render_block, "html").expect("find render/html decl");
    let (_, render_md_span) = find_decl(render_block, "md").expect("find render/md decl");
    let (_, render_svg_span) = find_decl(render_block, "svg").expect("find render/svg decl");

    engine_state
        .merge_delta(working_set.delta)
        .expect("merge nuju module delta");

    KernelInternalSpans {
        render: KernelInternalRenderSpans {
            text: render_text_span,
            csv: render_csv_span,
            json: render_json_span,
            html: render_html_span,
            md: render_md_span,
            svg: render_svg_span,
        },
    }
}

fn find_inner_block(block: &Block, name: &str) -> Option<BlockId> {
    for pipeline in block.pipelines.iter() {
        for element in pipeline.elements.iter() {
            if let Expr::Call(call) = &element.expr.expr {
                if let Some(Argument::Positional(expr)) = call.arguments.first() {
                    if let Expr::String(positional_name) = &expr.expr {
                        if positional_name == name {
                            if let Some(Argument::Positional(expr)) = call.arguments.get(1) {
                                if let Expr::Block(block_id) = &expr.expr {
                                    return Some(*block_id);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

fn find_decl(block: &Block, name: &str) -> Option<(DeclId, Span)> {
    for pipeline in block.pipelines.iter() {
        for element in pipeline.elements.iter() {
            if let Expr::AttributeBlock(attribute_block) = &element.expr.expr {
                if let Expr::Call(call) = &attribute_block.item.expr {
                    if let Some(Argument::Positional(expr)) = call.arguments.first() {
                        if let Expr::String(positional_name) = &expr.expr {
                            if positional_name == name {
                                // TODO: return expression span if it includes the `@`
                                return Some((call.decl_id, attribute_block.item.span));
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adding_nuju_module_works() {
        let engine_state = EngineState::default();
        let mut engine_state = nu_cmd_lang::create_default_context();
        let spans = create_nuju_module(&mut engine_state);
        assert_eq!(
            engine_state.get_span_contents(spans.render.json),
            br#"def "json" []: any -> string {...}"#
        )
    }
}
