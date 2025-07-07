use nu_plugin::{Plugin, PluginCommand};

use crate::commands;

pub struct PlottersPlugin;

impl Plugin for PlottersPlugin {
    fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").into()
    }

    fn commands(&self) -> Vec<Box<dyn PluginCommand<Plugin = Self>>> {
        vec![
            Box::new(commands::series::BarSeries),
            Box::new(commands::series::LineSeries),
            Box::new(commands::Chart2d),
            Box::new(commands::draw::DrawPng),
            Box::new(commands::draw::DrawSvg),
            Box::new(commands::draw::DrawTerminal),
        ]
    }
}
