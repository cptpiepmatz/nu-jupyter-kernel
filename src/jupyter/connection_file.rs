use std::fmt::Display;
use std::net::Ipv4Addr;
use std::path::Path;
use std::{fs, io};

use serde::{Deserialize, Deserializer};
use thiserror::Error;

#[derive(Debug, Deserialize)]
pub struct ConnectionFile {
    pub control_port: PortAddr,
    pub shell_port: PortAddr,
    pub transport: Transport,
    pub signature_scheme: SignatureScheme,
    pub stdin_port: PortAddr,
    #[serde(alias = "hb_port")]
    pub heartbeat_port: PortAddr,
    pub ip: Ipv4Addr,
    pub iopub_port: PortAddr,
    #[serde(deserialize_with = "deserialize_key")]
    pub key: Vec<u8>,
}

#[derive(Debug, Error)]
pub enum ReadConnectionFileError {
    #[error("could not read connection file")]
    ReadFile(#[from] io::Error),
    #[error("could not parse connection file")]
    Parse(#[from] serde_json::Error),
}

impl ConnectionFile {
    pub fn from_path(path: impl AsRef<Path>) -> Result<ConnectionFile, ReadConnectionFileError> {
        let contents = fs::read_to_string(path)?;
        let connection_file: ConnectionFile = serde_json::from_str(&contents)?;
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

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transport {
    Tcp,
}

impl Display for Transport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Transport::Tcp => write!(f, "tcp"),
        }
    }
}

fn deserialize_key<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let as_str = String::deserialize(deserializer)?;
    Ok(as_str.into_bytes())
}
