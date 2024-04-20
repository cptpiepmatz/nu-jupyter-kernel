use std::fmt::Display;
use std::fs;
use std::net::Ipv4Addr;
use std::path::Path;
use std::str::FromStr;

use serde::{Deserialize, Deserializer};

#[derive(Debug, Deserialize)]
pub struct ConnectionFile {
    pub control_port: PortAddr,
    pub shell_port: PortAddr,
    #[serde(deserialize_with = "deserialize_zmq_transport")]
    pub transport: zeromq::Transport,
    pub signature_scheme: SignatureScheme,
    pub stdin_port: PortAddr,
    #[serde(alias = "hb_port")]
    pub heartbeat_port: PortAddr,
    pub ip: Ipv4Addr,
    pub iopub_port: PortAddr,
    pub key: Key,
}

impl ConnectionFile {
    pub fn from_path(path: impl AsRef<Path>) -> Result<ConnectionFile, ()> {
        let contents = fs::read_to_string(path).unwrap();
        let connection_file: ConnectionFile = serde_json::from_str(&contents).unwrap();
        Ok(connection_file)
    }
}

#[derive(Debug, Deserialize)]
pub struct PortAddr(u16);

impl Display for PortAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Deserialize)]
pub struct Key(String);

#[derive(Debug)]
pub struct SignatureScheme {
    pub algorithm: SupportedSignatureAlgorithm,
    pub hash_fn: SupportedSignatureHashFunction,
}

impl<'de> Deserialize<'de> for SignatureScheme {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let as_str = String::deserialize(deserializer)?;
        let mut split = as_str.split('-');
        let algorithm = split
            .next()
            .ok_or_else(|| serde::de::Error::missing_field("algorithm"))?;
        let hash_fn = split
            .next()
            .ok_or_else(|| serde::de::Error::missing_field("hash_fn"))?;

        let algorithm = match algorithm {
            "hmac" => SupportedSignatureAlgorithm::Hmac,
            other => return Err(serde::de::Error::unknown_variant(other, &["hmac"])),
        };

        let hash_fn = match hash_fn {
            "sha256" => SupportedSignatureHashFunction::Sha256,
            other => return Err(serde::de::Error::unknown_variant(other, &["sha256"])),
        };

        Ok(SignatureScheme { algorithm, hash_fn })
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy)]
pub enum SupportedSignatureAlgorithm {
    Hmac,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy)]
pub enum SupportedSignatureHashFunction {
    Sha256,
}

fn deserialize_zmq_transport<'de, D>(deserializer: D) -> Result<zeromq::Transport, D::Error>
where
    D: Deserializer<'de>,
{
    let as_str = String::deserialize(deserializer)?;
    zeromq::Transport::from_str(&as_str).map_err(serde::de::Error::custom)
}
