use serde::Serialize;

use crate::CARGO_TOML;

#[derive(Debug, Serialize, Clone)]
pub struct KernelInfo {
    pub protocol_version: String,
    pub implementation: String,
    pub implementation_version: String,
    pub language_info: LanguageInfo,
    pub banner: String,
    pub debugger: bool,
    pub help_links: Vec<HelpLink>,
}

#[derive(Debug, Serialize, Clone)]
pub struct LanguageInfo {
    pub name: String,
    pub version: String,
    pub mimetype: String,
    pub file_extension: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct HelpLink {
    pub text: String,
    pub url: String,
}

impl<T, U> From<(T, U)> for HelpLink
where
    T: Into<String>,
    U: Into<String>,
{
    fn from(value: (T, U)) -> Self {
        HelpLink {
            text: value.0.into(),
            url: value.1.into(),
        }
    }
}

impl KernelInfo {
    pub fn get() -> Self {
        KernelInfo {
            protocol_version: CARGO_TOML
                .package
                .metadata
                .jupyter
                .protocol_version
                .to_owned(),
            implementation: CARGO_TOML.package.name.to_owned(),
            implementation_version: CARGO_TOML.package.version.to_owned(),
            language_info: LanguageInfo {
                name: "nushell".to_owned(),
                version: CARGO_TOML.workspace.dependencies.nu_engine.version.to_owned(),
                // TODO: verify this
                mimetype: "text/nu".to_owned(),
                file_extension: ".nu".to_owned(),
            },
            banner: include_str!("../../banner.txt").to_owned(),
            debugger: false,
            help_links: [
                ("Discord", "https://discord.gg/NtAbbGn"),
                ("GitHub", "https://github.com/nushell/nushell"),
            ]
            .into_iter()
            .map(|pair| pair.into())
            .collect(),
        }
    }
}
