#[cfg(not(feature = "nu-plugin"))]
compile_error!(indoc::indoc! {"
    The `nu-plugin` feature is required to built plugin. 
    Ensure default features are enabled for plugin install.
"});

fn main() {
    #[cfg(feature = "nu-plugin")]
    nu_plugin::serve_plugin(
        &nu_plugin_plotters::plugin::PlottersPlugin,
        nu_plugin::MsgPackSerializer,
    )
}
