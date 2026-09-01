use std::{
    sync::mpsc::{self, RecvTimeoutError},
    thread,
    time::Duration,
};

use chrono::{DateTime, Utc};
use synchalo_core::{AppError, MAX_CLIPBOARD_BYTES, content_hash};
use tokio::sync::mpsc as tokio_mpsc;

#[derive(Debug, Clone)]
pub struct ClipboardObservation {
    pub text: String,
    pub observed_at: DateTime<Utc>,
}

enum ClipboardCommand {
    SetText(String),
    Stop,
}

pub struct ClipboardMonitor {
    commands: mpsc::Sender<ClipboardCommand>,
    observations: Option<tokio_mpsc::UnboundedReceiver<Result<ClipboardObservation, AppError>>>,
    thread: Option<thread::JoinHandle<()>>,
}

impl ClipboardMonitor {
    pub fn start(poll_interval: Duration) -> Result<Self, AppError> {
        let (commands_tx, commands_rx) = mpsc::channel();
        let (observations_tx, observations_rx) = tokio_mpsc::unbounded_channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);

        let thread = thread::Builder::new()
            .name("synchalo-clipboard".to_owned())
            .spawn(move || {
                let mut clipboard = match arboard::Clipboard::new() {
                    Ok(clipboard) => {
                        let _ = ready_tx.send(Ok(()));
                        clipboard
                    }
                    Err(error) => {
                        let message = error.to_string();
                        let _ = ready_tx.send(Err(message.clone()));
                        let _ = observations_tx.send(Err(AppError::Clipboard(message)));
                        return;
                    }
                };

                let mut last_hash = clipboard
                    .get_text()
                    .ok()
                    .map(|text| content_hash(text.as_bytes()));

                loop {
                    match commands_rx.recv_timeout(poll_interval) {
                        Ok(ClipboardCommand::SetText(text)) => {
                            last_hash = Some(content_hash(text.as_bytes()));
                            if let Err(error) = clipboard.set_text(text) {
                                let _ = observations_tx
                                    .send(Err(AppError::Clipboard(error.to_string())));
                            }
                        }
                        Ok(ClipboardCommand::Stop) => break,
                        Err(RecvTimeoutError::Disconnected) => break,
                        Err(RecvTimeoutError::Timeout) => match clipboard.get_text() {
                            Ok(text) if !text.is_empty() && text.len() <= MAX_CLIPBOARD_BYTES => {
                                let hash = content_hash(text.as_bytes());
                                if last_hash.as_deref() != Some(hash.as_str()) {
                                    last_hash = Some(hash);
                                    let _ = observations_tx.send(Ok(ClipboardObservation {
                                        text,
                                        observed_at: Utc::now(),
                                    }));
                                }
                            }
                            Ok(_) => {}
                            Err(arboard::Error::ContentNotAvailable) => {}
                            Err(error) => {
                                tracing::debug!(%error, "clipboard poll failed");
                            }
                        },
                    }
                }
            })
            .map_err(|error| AppError::Clipboard(error.to_string()))?;

        match ready_rx.recv_timeout(Duration::from_secs(3)) {
            Ok(Ok(())) => Ok(Self {
                commands: commands_tx,
                observations: Some(observations_rx),
                thread: Some(thread),
            }),
            Ok(Err(message)) => {
                let _ = thread.join();
                Err(AppError::Clipboard(message))
            }
            Err(error) => Err(AppError::Clipboard(format!(
                "clipboard initialization timed out: {error}"
            ))),
        }
    }

    pub fn take_observations(
        &mut self,
    ) -> Option<tokio_mpsc::UnboundedReceiver<Result<ClipboardObservation, AppError>>> {
        self.observations.take()
    }

    pub fn set_text(&self, text: impl Into<String>) -> Result<(), AppError> {
        self.commands
            .send(ClipboardCommand::SetText(text.into()))
            .map_err(|_| AppError::Clipboard("clipboard worker has stopped".to_owned()))
    }
}

impl Drop for ClipboardMonitor {
    fn drop(&mut self) {
        let _ = self.commands.send(ClipboardCommand::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
