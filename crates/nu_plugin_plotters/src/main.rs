fn main() {
    nu_plugin::serve_plugin(
        &nu_plugin_plotters::plugin::PlottersPlugin,
        nu_plugin::MsgPackSerializer,
    )
}
