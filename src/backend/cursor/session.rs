//! Cursor Agent spawn-per-turn session management.
//!
//! Cursor Agent CLI does not support a long-lived server mode. Multi-turn
//! sessions are achieved by spawning a new process for each turn using
//! `agent --print --resume <chatId>`.

use crate::error::{Error, Result};
use crate::options::AgentOptions;
use crate::types::{Message, Prompt};
use async_stream::stream;
use futures::Stream;
use std::pin::Pin;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::broadcast;

use super::message_parser;
use super::transport::find_cursor_cli;

const MESSAGE_BUFFER_SIZE: usize = 100;

/// Internal control message for the cursor session.
#[derive(Debug, Clone)]
enum SessionMessage {
    SdkMessage(Message),
    End,
    Error(String),
}

/// Multi-turn session for Cursor Agent using spawn-per-turn.
///
/// Each call to [`send_message`](CursorSession::send_message) spawns a new
/// `agent --print --resume <chatId>` process. The `chatId` is extracted
/// from the first turn's `system/init` event.
pub struct CursorSession {
    cli_path: String,
    options: AgentOptions,
    chat_id: Option<String>,
    message_tx: broadcast::Sender<SessionMessage>,
}

impl std::fmt::Debug for CursorSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CursorSession")
            .field("cli_path", &self.cli_path)
            .field("chat_id", &self.chat_id)
            .finish_non_exhaustive()
    }
}

impl CursorSession {
    /// Create a new Cursor session, optionally running an initial prompt.
    pub async fn new(options: &AgentOptions, prompt: Option<Prompt>) -> Result<Self> {
        let cli_path = find_cursor_cli(options)?;
        let (message_tx, _) = broadcast::channel(MESSAGE_BUFFER_SIZE);

        let mut session = Self {
            cli_path,
            options: options.clone(),
            chat_id: None,
            message_tx,
        };

        if let Some(prompt) = prompt {
            let prompt_text = match prompt {
                Prompt::Text(s) => s,
                Prompt::Stream(_) => {
                    return Err(Error::Other(
                        "Cursor session does not support stream prompts".to_string(),
                    ));
                }
            };
            session.run_turn(&prompt_text).await?;
        }

        Ok(session)
    }

    /// Spawn a new agent process for one turn.
    async fn run_turn(&mut self, prompt: &str) -> Result<()> {
        let mut cmd_args = vec![
            "--print".to_string(),
            "--output-format".to_string(),
            "stream-json".to_string(),
        ];

        if let Some(ref chat_id) = self.chat_id {
            cmd_args.push("--resume".to_string());
            cmd_args.push(chat_id.clone());
        }

        if let Some(ref m) = self.options.model {
            cmd_args.push("--model".to_string());
            cmd_args.push(m.clone());
        }

        if let Some(ref cursor_opts) = self.options.cursor {
            if cursor_opts.force_approve {
                cmd_args.push("--force".to_string());
            }
            if let Some(ref mode) = cursor_opts.mode {
                cmd_args.push("--mode".to_string());
                cmd_args.push(mode.clone());
            }
            if cursor_opts.trust_workspace {
                cmd_args.push("--trust".to_string());
            }
        }

        for (key, value) in &self.options.extra_args {
            if let Some(v) = value {
                cmd_args.push(format!("--{}", key));
                cmd_args.push(v.clone());
            } else {
                cmd_args.push(format!("--{}", key));
            }
        }

        // Prompt goes last
        cmd_args.push(prompt.to_string());

        let mut child_cmd = tokio::process::Command::new(&self.cli_path);
        child_cmd
            .args(&cmd_args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        if let Some(ref cwd) = self.options.cwd {
            child_cmd.current_dir(cwd);
        }

        for (k, v) in &self.options.env {
            child_cmd.env(k, v);
        }

        let mut process = child_cmd.spawn().map_err(|e| {
            Error::CliNotFound(format!(
                "Cursor Agent CLI not found at: {} - {}",
                self.cli_path, e
            ))
        })?;

        let stdout = process
            .stdout
            .take()
            .ok_or_else(|| Error::Other("Failed to capture stdout".to_string()))?;

        let msg_tx = self.message_tx.clone();

        // Spawn a reader task for this turn
        let chat_id_holder: std::sync::Arc<tokio::sync::Mutex<Option<String>>> =
            std::sync::Arc::new(tokio::sync::Mutex::new(self.chat_id.clone()));
        let chat_id_for_task = chat_id_holder.clone();

        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                let data: serde_json::Value = match serde_json::from_str(line) {
                    Ok(d) => d,
                    Err(_) => continue,
                };

                // Extract chatId from system/init events
                if data.get("type").and_then(|v| v.as_str()) == Some("system")
                    && let Some(chat_id) = data
                        .get("chatId")
                        .or_else(|| data.get("session_id"))
                        .and_then(|v| v.as_str())
                {
                    *chat_id_for_task.lock().await = Some(chat_id.to_string());
                }

                // Also extract from result messages
                if data.get("type").and_then(|v| v.as_str()) == Some("result")
                    && let Some(session_id) = data.get("session_id").and_then(|v| v.as_str())
                {
                    let mut id_lock = chat_id_for_task.lock().await;
                    if id_lock.is_none() {
                        *id_lock = Some(session_id.to_string());
                    }
                }

                match message_parser::parse_cursor_event(&data) {
                    Ok(Some(msg)) => {
                        let _ = msg_tx.send(SessionMessage::SdkMessage(msg));
                    }
                    Ok(None) => {}
                    Err(e) => {
                        let _ = msg_tx.send(SessionMessage::Error(format!("Parse error: {}", e)));
                    }
                }
            }

            // Wait for process exit
            let exit_status = process.wait().await;
            if let Ok(status) = exit_status
                && !status.success()
            {
                let _ = msg_tx.send(SessionMessage::Error(format!(
                    "Process exited with code {}",
                    status.code().unwrap_or(-1)
                )));
            }

            let _ = msg_tx.send(SessionMessage::End);
        });

        // Give the reader task a moment to extract the chatId
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        if let Some(id) = chat_id_holder.lock().await.clone() {
            self.chat_id = Some(id);
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl crate::backend::Session for CursorSession {
    async fn send_message(&mut self, prompt: Prompt, _session_id: &str) -> Result<()> {
        let prompt_text = match prompt {
            Prompt::Text(s) => s,
            Prompt::Stream(_) => {
                return Err(Error::Other(
                    "Cursor session does not support stream prompts. Use Prompt::Text.".to_string(),
                ));
            }
        };
        self.run_turn(&prompt_text).await
    }

    fn receive_messages(&self) -> Pin<Box<dyn Stream<Item = Result<Message>> + Send + '_>> {
        let mut rx = self.message_tx.subscribe();

        let stream = stream! {
            loop {
                match rx.recv().await {
                    Ok(SessionMessage::SdkMessage(msg)) => yield Ok(msg),
                    Ok(SessionMessage::End) => break,
                    Ok(SessionMessage::Error(e)) => {
                        yield Err(Error::Other(e));
                        break;
                    }
                    Err(_) => break,
                }
            }
        };

        Box::pin(stream)
    }

    fn receive_response(&self) -> Pin<Box<dyn Stream<Item = Result<Message>> + Send + '_>> {
        let mut rx = self.message_tx.subscribe();

        let stream = stream! {
            loop {
                match rx.recv().await {
                    Ok(SessionMessage::SdkMessage(msg)) => {
                        let is_result = matches!(&msg, Message::Result(_));
                        yield Ok(msg);
                        if is_result {
                            break;
                        }
                    }
                    Ok(SessionMessage::End) => break,
                    Ok(SessionMessage::Error(e)) => {
                        yield Err(Error::Other(e));
                        break;
                    }
                    Err(_) => break,
                }
            }
        };

        Box::pin(stream)
    }

    async fn send_control_request(
        &mut self,
        request: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let subtype = request
            .get("subtype")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        Err(Error::UnsupportedFeature {
            feature: format!("control request '{}'", subtype),
            backend: "Cursor".to_string(),
        })
    }

    async fn get_server_info(&self) -> Option<serde_json::Value> {
        self.chat_id
            .as_ref()
            .map(|id| serde_json::json!({"chatId": id}))
    }

    async fn close(&mut self) -> Result<()> {
        Ok(())
    }
}
