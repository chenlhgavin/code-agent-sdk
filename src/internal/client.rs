//! Internal client implementation.

use crate::error::{Error, Result};
use crate::internal::query::Query;
use crate::options::ClaudeAgentOptions;
use crate::transport::{SubprocessCliTransport, Transport};
use crate::types::{Message, Prompt};
use async_stream::stream;
use futures::Stream;
use std::pin::Pin;

pub struct InternalClient;

impl InternalClient {
    pub fn new() -> Self {
        Self
    }

    pub fn process_query(
        &self,
        prompt: Prompt,
        options: ClaudeAgentOptions,
    ) -> Pin<Box<dyn Stream<Item = Result<Message>> + Send>> {
        let stream = stream! {
            // Validate: can_use_tool requires Stream prompt, not Text
            if options.can_use_tool.is_some() && matches!(&prompt, Prompt::Text(_)) {
                yield Err(Error::Other(
                    "can_use_tool callback requires a Stream prompt, not a string prompt. \
                     Use Prompt::Stream for bidirectional communication."
                        .to_string(),
                ));
                return;
            }

            // Validate and configure permission settings (matching Python SDK logic)
            let mut configured_options = options.clone();
            if configured_options.can_use_tool.is_some() {
                // canUseTool and permission_prompt_tool_name are mutually exclusive
                if configured_options.permission_prompt_tool_name.is_some() {
                    yield Err(Error::Other(
                        "can_use_tool callback cannot be used with permission_prompt_tool_name. \
                         Please use one or the other."
                            .to_string(),
                    ));
                    return;
                }
                // Automatically set permission_prompt_tool_name to "stdio" for control protocol
                configured_options.permission_prompt_tool_name = Some("stdio".to_string());
            }

            let prompt_str = match &prompt {
                Prompt::Text(s) => s.clone(),
                Prompt::Stream(_) => String::new(),
            };

            let mut transport = match SubprocessCliTransport::new(&prompt_str, configured_options.clone()) {
                Ok(t) => t,
                Err(e) => {
                    yield Err(e);
                    return;
                }
            };

            if let Err(e) = transport.connect().await {
                yield Err(e);
                return;
            }

            let transport: Box<dyn crate::transport::Transport + Send> = Box::new(transport);

            // Create Query to handle control protocol (hooks, can_use_tool, mcp_servers)
            let mut query = Query::new(transport, &configured_options);

            if let Err(e) = query.initialize(&configured_options).await {
                yield Err(e);
                let _ = query.close().await;
                return;
            }

            match prompt {
                Prompt::Text(ref text) => {
                    // Send user message
                    if let Err(e) = query.write_user_message(text, "").await {
                        yield Err(e);
                        let _ = query.close().await;
                        return;
                    }

                    // End input (for one-shot queries, close stdin after sending user message)
                    if let Err(e) = query.end_input().await {
                        yield Err(e);
                        let _ = query.close().await;
                        return;
                    }
                }
                Prompt::Stream(input_stream) => {
                    // Stream input messages to the query
                    if let Err(e) = query.stream_input(input_stream).await {
                        yield Err(e);
                        let _ = query.close().await;
                        return;
                    }
                }
            }

            // Yield parsed messages from the query's receive_response stream
            {
                use futures::StreamExt;
                let mut response_stream = query.receive_response();
                while let Some(item) = response_stream.next().await {
                    match item {
                        Ok(msg) => {
                            yield Ok(msg);
                        }
                        Err(e) => {
                            yield Err(e);
                            break;
                        }
                    }
                }
            }
            // response_stream is dropped here, releasing the borrow on query

            let _ = query.close().await;
        };

        Box::pin(stream)
    }
}

impl Default for InternalClient {
    fn default() -> Self {
        Self::new()
    }
}
