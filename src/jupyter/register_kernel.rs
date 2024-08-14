use std::path::{Path, PathBuf};
use std::{env, fs};

use clap::ValueEnum;
use serde_json::json;

#[derive(Debug, ValueEnum, Clone, Copy)]
pub enum RegisterLocation {
    User,
    System,
}

pub fn register_kernel(location: RegisterLocation) {
    let path = kernel_path(location);
    let path = path.as_ref();
    // TODO: handle errors well here
    fs::create_dir_all(path).unwrap();
    let mut file_path = PathBuf::from(path);
    file_path.push("kernel.json");
    let manifest = serde_json::to_string_pretty(&kernel_manifest()).unwrap();
    fs::write(&file_path, manifest).unwrap();
    println!("Registered kernel to {}", path.display());
}

fn kernel_path(location: RegisterLocation) -> impl AsRef<Path> {
    let mut path = PathBuf::new();

    #[cfg(target_os = "windows")]
    match location {
        RegisterLocation::User => {
            let appdata = env::var("APPDATA").expect("%APPDATA% not found");
            path.push(appdata);
            path.push(r"jupyter\kernels");
        }
        RegisterLocation::System => {
            let programdata = env::var("PROGRAMDATA").expect("%PROGRAMDATA% not found");
            path = PathBuf::from(programdata);
            path.push(r"jupyter\kernels");
        }
    }

    #[cfg(target_os = "linux")]
    match location {
        RegisterLocation::User => {
            path.push(dirs::home_dir().expect("defined on linux"));
            path.push(".local/share/jupyter/kernels")
        }
        RegisterLocation::System => path.push("/usr/local/share/jupyter/kernels"),
    }

    #[cfg(target_os = "macos")]
    match location {
        RegisterLocation::User => {
            path.push(dirs::home_dir().expect("defined on macos"));
            path.push("Library/Jupyter/kernels")
        }
        RegisterLocation::System => path.push("/usr/local/share/jupyter/kernels"),
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    panic!(
        "Your target os ({}) doesn't support `register`",
        env::consts::OS
    );

    path.push("nu");

    path
}

fn kernel_manifest() -> serde_json::Value {
    let this_exec = env::current_exe().unwrap();
    json!({
        "argv": [this_exec, "start", "{connection_file}"],
        "display_name": "Nushell",
        "language": "nushell",
        "interrupt_mode": "signal", // TODO: make this "message" later
        "env": {},
        "metadata": {}
    })
}
