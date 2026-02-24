//! `ClaudeSdkClient` - bidirectional streaming client.

use crate::error::{Error, Result};
use crate::internal::query::Query;
use crate::options::ClaudeAgentOptions;
use crate::transport::{SubprocessCliTransport, Transport};
use crate::types::{Message, Prompt};
use futures::Stream;
use std::pin::Pin;

/// Client for bidirectional, interactive conversations with Claude Code.
pub struct ClaudeSdkClient {
    options: ClaudeAgentOptions,
    custom_transport: Option<Box<dyn Transport + Send>>,
    query: Option<Query>,
}

impl ClaudeSdkClient {
    /// Create a new client with optional configuration and custom transport.
    ///
    /// # Arguments
    /// * `options` - Configuration options (uses defaults if `None`).
    /// * `custom_transport` - Optional custom transport (uses subprocess CLI if `None`).
    pub fn new(
        options: Option<ClaudeAgentOptions>,
        custom_transport: Option<Box<dyn Transport + Send>>,
    ) -> Self {
        Self {
            options: options.unwrap_or_default(),
            custom_transport,
            query: None,
        }
    }

    /// Connect to Claude Code.
    ///
    /// Matching Python SDK behavior:
    /// - `None` prompt: connects without sending a message (interactive mode).
    /// - `Prompt::Text`: NOT auto-sent. Use `query()` after connecting.
    /// - `Prompt::Stream`: starts background stream input task.
    pub async fn connect(&mut self, prompt: Option<Prompt>) -> Result<()> {
        if self.query.is_some() {
            return Ok(());
        }

        // Validate: can_use_tool requires Stream prompt, not Text
        if self.options.can_use_tool.is_some() {
            if let Some(Prompt::Text(_)) = &prompt {
                return Err(Error::Other(
                    "can_use_tool callback requires a Stream prompt, not a string prompt. \
                     Use Prompt::Stream for bidirectional communication."
                        .to_string(),
                ));
            }
        }

        // Validate and configure permission settings (matching Python SDK logic)
        let mut options = self.options.clone();
        if options.can_use_tool.is_some() {
            // canUseTool and permission_prompt_tool_name are mutually exclusive
            if options.permission_prompt_tool_name.is_some() {
                return Err(Error::Other(
                    "can_use_tool callback cannot be used with permission_prompt_tool_name. \
                     Please use one or the other."
                        .to_string(),
                ));
            }
            // Automatically set permission_prompt_tool_name to "stdio" for control protocol
            options.permission_prompt_tool_name = Some("stdio".to_string());
        }

        let transport: Box<dyn Transport + Send> = if let Some(t) = self.custom_transport.take() {
            t
        } else {
            let mut transport = SubprocessCliTransport::new("", options.clone())?;
            transport.connect().await?;
            Box::new(transport)
        };

        let mut query = Query::new(transport, &options);
        query.initialize(&options).await?;

        // Handle Stream prompt: start background stream input
        if let Some(Prompt::Stream(input_stream)) = prompt {
            query.stream_input(input_stream).await?;
        }
        // Note: Prompt::Text is NOT auto-sent on connect (matching Python SDK).
        // Use query() to send messages after connecting.

        self.query = Some(query);
        Ok(())
    }

    /// Send a query to Claude Code.
    ///
    /// For `Prompt::Text`: writes a user message with the given session_id.
    /// For `Prompt::Stream`: iterates and writes each message with session_id injection.
    pub async fn query(&mut self, prompt: impl Into<Prompt>, session_id: &str) -> Result<()> {
        let query = self.query.as_mut().ok_or(Error::NotConnected)?;

        match prompt.into() {
            Prompt::Text(text) => query.write_user_message(&text, session_id).await,
            Prompt::Stream(input_stream) => query.stream_input(input_stream).await,
        }
    }

    pub fn receive_messages(&self) -> Pin<Box<dyn Stream<Item = Result<Message>> + Send + '_>> {
        if let Some(ref q) = self.query {
            q.receive_messages()
        } else {
            Box::pin(futures::stream::once(async { Err(Error::NotConnected) }))
        }
    }

    pub fn receive_response(&self) -> Pin<Box<dyn Stream<Item = Result<Message>> + Send + '_>> {
        if let Some(ref q) = self.query {
            q.receive_response()
        } else {
            Box::pin(futures::stream::once(async { Err(Error::NotConnected) }))
        }
    }

    pub async fn interrupt(&mut self) -> Result<()> {
        let query = self.query.as_mut().ok_or(Error::NotConnected)?;
        query
            .send_control_request(serde_json::json!({"subtype": "interrupt"}))
            .await?;
        Ok(())
    }

    pub async fn set_permission_mode(&mut self, mode: &str) -> Result<()> {
        let query = self.query.as_mut().ok_or(Error::NotConnected)?;
        query
            .send_control_request(serde_json::json!({
                "subtype": "set_permission_mode",
                "mode": mode
            }))
            .await?;
        Ok(())
    }

    pub async fn set_model(&mut self, model: Option<&str>) -> Result<()> {
        let query = self.query.as_mut().ok_or(Error::NotConnected)?;
        query
            .send_control_request(serde_json::json!({
                "subtype": "set_model",
                "model": model
            }))
            .await?;
        Ok(())
    }

    pub async fn rewind_files(&mut self, user_message_id: &str) -> Result<()> {
        let query = self.query.as_mut().ok_or(Error::NotConnected)?;
        query
            .send_control_request(serde_json::json!({
                "subtype": "rewind_files",
                "user_message_id": user_message_id
            }))
            .await?;
        Ok(())
    }

    pub async fn get_mcp_status(&mut self) -> Result<serde_json::Value> {
        let query = self.query.as_mut().ok_or(Error::NotConnected)?;
        query
            .send_control_request(serde_json::json!({"subtype": "mcp_status"}))
            .await
    }

    pub async fn get_server_info(&self) -> Result<Option<serde_json::Value>> {
        if let Some(ref q) = self.query {
            Ok(q.get_server_info().await)
        } else {
            Err(Error::NotConnected)
        }
    }

    pub async fn disconnect(&mut self) -> Result<()> {
        if let Some(mut q) = self.query.take() {
            q.close().await?;
        }
        Ok(())
    }
}
