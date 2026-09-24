use std::fs::{self, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::time::Duration;

use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, Serialize)]
pub struct AuditEvent {
    pub request_id: String,
    pub operation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<String>,
    pub outcome: String,
    pub elapsed_micros: u64,
}

#[derive(Debug, Error)]
pub enum AuditError {
    #[error("cannot initialize audit log: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Clone)]
pub struct AuditLogger {
    sender: SyncSender<AuditEvent>,
}

impl AuditLogger {
    pub fn start(path: PathBuf) -> Result<Self, AuditError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let (sender, receiver) = mpsc::sync_channel(1024);
        std::thread::Builder::new()
            .name("db-operator-audit".to_owned())
            .spawn(move || write_loop(BufWriter::new(file), receiver))?;
        Ok(Self { sender })
    }

    pub fn record(&self, event: AuditEvent) {
        let _ = self.sender.try_send(event);
    }
}

fn write_loop(mut writer: BufWriter<std::fs::File>, receiver: Receiver<AuditEvent>) {
    loop {
        let first = match receiver.recv_timeout(Duration::from_millis(500)) {
            Ok(event) => event,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        if write_event(&mut writer, &first).is_err() {
            break;
        }
        while let Ok(event) = receiver.try_recv() {
            if write_event(&mut writer, &event).is_err() {
                return;
            }
        }
        if writer.flush().is_err() {
            break;
        }
    }
    let _ = writer.flush();
}

fn write_event(writer: &mut BufWriter<std::fs::File>, event: &AuditEvent) -> std::io::Result<()> {
    serde_json::to_writer(&mut *writer, event).map_err(std::io::Error::other)?;
    writer.write_all(b"\n")
}
