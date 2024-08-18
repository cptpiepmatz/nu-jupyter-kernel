use nu_plugin::{Plugin, PluginCommand};

pub struct PlottersPlugin;

impl Plugin for PlottersPlugin {
    fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").into()
    }

    fn commands(&self) -> Vec<Box<dyn PluginCommand<Plugin = Self>>> {
        todo!()
    }
}
