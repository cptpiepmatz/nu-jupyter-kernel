use serde::{Deserialize, Serialize};

pub mod connection_file;
pub mod kernel_info;
pub mod messages;
pub mod register_kernel;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Shutdown {
    pub restart: bool,
}
