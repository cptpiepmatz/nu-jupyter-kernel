use std::sync::mpsc;

use crate::jupyter::messages::multipart::Multipart;

pub struct Print(mpsc::Sender<Multipart>);
