//! Subprocess transport using Claude Code CLI.

use crate::error::{Error, Result};
use crate::options::ClaudeAgentOptions;
use crate::transport::Transport;
use async_stream::stream;
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

const DEFAULT_MAX_BUFFER_SIZE: usize = 1024 * 1024; // 1MB
#[allow(dead_code)]
const MINIMUM_CLAUDE_CODE_VERSION: &str = "2.0.0";

pub struct SubprocessCliTransport {
    options: ClaudeAgentOptions,
    cli_path: String,
    cwd: Option<String>,
    process: Option<Child>,
    stdin: Option<ChildStdin>,
    stdout: Option<ChildStdout>,
    ready: bool,
    exit_error: Option<Error>,
    max_buffer_size: usize,
}

impl SubprocessCliTransport {
    pub fn new(_prompt: &str, options: ClaudeAgentOptions) -> Result<Self> {
        let cli_path = Self::find_cli(&options)?;
        let cwd = options
            .cwd
            .as_ref()
            .map(|p| p.to_string_lossy().to_string());

        let max_buffer_size = options.max_buffer_size.unwrap_or(DEFAULT_MAX_BUFFER_SIZE);

        Ok(Self {
            options,
            cli_path,
            cwd,
            process: None,
            stdin: None,
            stdout: None,
            ready: false,
            exit_error: None,
            max_buffer_size,
        })
    }

    fn find_cli(options: &ClaudeAgentOptions) -> Result<String> {
        if let Some(ref p) = options.cli_path {
            return Ok(p.to_string_lossy().to_string());
        }

        if let Some(bundled) = Self::find_bundled_cli() {
            return Ok(bundled);
        }

        if let Some(path) = which_cli() {
            return Ok(path);
        }

        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let locations = [
            format!("{}/.npm-global/bin/claude", home),
            "/usr/local/bin/claude".to_string(),
            format!("{}/.local/bin/claude", home),
            format!("{}/node_modules/.bin/claude", home),
            format!("{}/.yarn/bin/claude", home),
            format!("{}/.claude/local/claude", home),
        ];

        for path in &locations {
            if Path::new(path).exists() {
                return Ok(path.clone());
            }
        }

        Err(Error::CliNotFound(
            "Claude Code not found. Install with:\n  npm install -g @anthropic-ai/claude-code\n\n\
             Or provide the path via ClaudeAgentOptions::cli_path()"
                .to_string(),
        ))
    }

    fn find_bundled_cli() -> Option<String> {
        let cli_name = if cfg!(target_os = "windows") {
            "claude.exe"
        } else {
            "claude"
        };

        let exe = std::env::current_exe().ok()?;
        let dir = exe.parent()?;
        let bundled = dir.join("_bundled").join(cli_name);

        if bundled.exists() {
            Some(bundled.to_string_lossy().to_string())
        } else {
            None
        }
    }

    fn build_command(&self) -> Vec<String> {
        let mut cmd = vec![
            self.cli_path.clone(),
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--verbose".to_string(),
        ];

        match &self.options.system_prompt {
            None => {
                cmd.push("--system-prompt".to_string());
                cmd.push(String::new());
            }
            Some(crate::options::SystemPromptConfig::String(s)) => {
                cmd.push("--system-prompt".to_string());
                cmd.push(s.clone());
            }
            Some(crate::options::SystemPromptConfig::Preset { append, .. }) => {
                if let Some(a) = append {
                    cmd.push("--append-system-prompt".to_string());
                    cmd.push(a.clone());
                }
            }
        }

        if let Some(ref tools) = self.options.tools {
            match tools {
                crate::options::ToolsConfig::List(list) => {
                    cmd.push("--tools".to_string());
                    cmd.push(if list.is_empty() {
                        String::new()
                    } else {
                        list.join(",")
                    });
                }
                crate::options::ToolsConfig::Preset { .. } => {
                    cmd.push("--tools".to_string());
                    cmd.push("default".to_string());
                }
            }
        }

        if !self.options.allowed_tools.is_empty() {
            cmd.push("--allowedTools".to_string());
            cmd.push(self.options.allowed_tools.join(","));
        }
        if let Some(t) = self.options.max_turns {
            cmd.push("--max-turns".to_string());
            cmd.push(t.to_string());
        }
        if let Some(b) = self.options.max_budget_usd {
            cmd.push("--max-budget-usd".to_string());
            cmd.push(b.to_string());
        }
        if !self.options.disallowed_tools.is_empty() {
            cmd.push("--disallowedTools".to_string());
            cmd.push(self.options.disallowed_tools.join(","));
        }
        if let Some(ref m) = self.options.model {
            cmd.push("--model".to_string());
            cmd.push(m.clone());
        }
        if let Some(ref m) = self.options.fallback_model {
            cmd.push("--fallback-model".to_string());
            cmd.push(m.clone());
        }
        if let Some(ref m) = self.options.permission_mode {
            cmd.push("--permission-mode".to_string());
            cmd.push(m.to_string());
        }
        if self.options.continue_conversation {
            cmd.push("--continue".to_string());
        }
        if let Some(ref r) = self.options.resume {
            cmd.push("--resume".to_string());
            cmd.push(r.clone());
        }
        if let Some(settings_value) = self.build_settings_value() {
            cmd.push("--settings".to_string());
            cmd.push(settings_value);
        }
        if !self.options.betas.is_empty() {
            cmd.push("--betas".to_string());
            let betas_str: Vec<String> = self.options.betas.iter().map(|b| b.to_string()).collect();
            cmd.push(betas_str.join(","));
        }
        // Note: `user` option is handled at process spawn time (uid), not as a CLI arg.
        // See connect() for the #[cfg(unix)] uid resolution.
        for dir in &self.options.add_dirs {
            cmd.push("--add-dir".to_string());
            cmd.push(dir.to_string_lossy().to_string());
        }
        match self.options.mcp_servers.as_ref() {
            Some(McpServersConfig::Dict(servers)) => {
                let mut for_cli = serde_json::Map::new();
                for (name, config) in servers {
                    let config_val = match config {
                        McpServerConfig::Sdk(c) => {
                            let mut m = serde_json::Map::new();
                            m.insert(
                                "type".to_string(),
                                serde_json::Value::String("sdk".to_string()),
                            );
                            m.insert(
                                "name".to_string(),
                                serde_json::Value::String(c.name.clone()),
                            );
                            m.insert(
                                "version".to_string(),
                                serde_json::Value::String(c.version.clone()),
                            );
                            // Note: tools/handlers are not sent to CLI - they run in-process.
                            serde_json::Value::Object(m)
                        }
                        McpServerConfig::Stdio(c) => {
                            let mut m = serde_json::Map::new();
                            m.insert("type".to_string(), serde_json::json!("stdio"));
                            m.insert("command".to_string(), serde_json::json!(c.command));
                            if let Some(ref args) = c.args {
                                m.insert(
                                    "args".to_string(),
                                    serde_json::to_value(args).unwrap_or_default(),
                                );
                            }
                            if let Some(ref env) = c.env {
                                let env_obj: serde_json::Map<_, _> = env
                                    .iter()
                                    .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                                    .collect();
                                m.insert("env".to_string(), serde_json::Value::Object(env_obj));
                            }
                            serde_json::Value::Object(m)
                        }
                        McpServerConfig::Sse(c) => {
                            let mut m = serde_json::Map::new();
                            m.insert("type".to_string(), serde_json::json!("sse"));
                            m.insert("url".to_string(), serde_json::json!(c.url));
                            if let Some(ref headers) = c.headers {
                                let headers_obj: serde_json::Map<_, _> = headers
                                    .iter()
                                    .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                                    .collect();
                                m.insert(
                                    "headers".to_string(),
                                    serde_json::Value::Object(headers_obj),
                                );
                            }
                            serde_json::Value::Object(m)
                        }
                        McpServerConfig::Http(c) => {
                            let mut m = serde_json::Map::new();
                            m.insert("type".to_string(), serde_json::json!("http"));
                            m.insert("url".to_string(), serde_json::json!(c.url));
                            if let Some(ref headers) = c.headers {
                                let headers_obj: serde_json::Map<_, _> = headers
                                    .iter()
                                    .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                                    .collect();
                                m.insert(
                                    "headers".to_string(),
                                    serde_json::Value::Object(headers_obj),
                                );
                            }
                            serde_json::Value::Object(m)
                        }
                    };
                    for_cli.insert(name.clone(), config_val);
                }
                if !for_cli.is_empty() {
                    let mcp_config = serde_json::json!({"mcpServers": for_cli});
                    cmd.push("--mcp-config".to_string());
                    cmd.push(mcp_config.to_string());
                }
            }
            Some(McpServersConfig::Path(path)) => {
                cmd.push("--mcp-config".to_string());
                cmd.push(path.clone());
            }
            None => {}
        }
        if self.options.include_partial_messages {
            cmd.push("--include-partial-messages".to_string());
        }
        if self.options.fork_session {
            cmd.push("--fork-session".to_string());
        }
        {
            let sources_value = match self.options.setting_sources.as_ref() {
                Some(sources) => {
                    let strs: Vec<String> = sources.iter().map(|s| s.to_string()).collect();
                    strs.join(",")
                }
                None => String::new(),
            };
            cmd.push("--setting-sources".to_string());
            cmd.push(sources_value);
        }
        for plugin in &self.options.plugins {
            if plugin.type_ == "local" {
                cmd.push("--plugin-dir".to_string());
                cmd.push(plugin.path.clone());
            }
        }
        for (flag, value) in &self.options.extra_args {
            if let Some(v) = value {
                cmd.push(format!("--{}", flag));
                cmd.push(v.clone());
            } else {
                cmd.push(format!("--{}", flag));
            }
        }
        let resolved_thinking = self.resolve_max_thinking_tokens();
        if let Some(tokens) = resolved_thinking {
            cmd.push("--max-thinking-tokens".to_string());
            cmd.push(tokens.to_string());
        }
        if let Some(ref e) = self.options.effort {
            cmd.push("--effort".to_string());
            cmd.push(e.to_string());
        }
        if let Some(ref of) = self.options.output_format {
            if let Some(obj) = of.as_object() {
                if obj.get("type").and_then(|v| v.as_str()) == Some("json_schema") {
                    if let Some(schema) = obj.get("schema") {
                        cmd.push("--json-schema".to_string());
                        cmd.push(schema.to_string());
                    }
                }
            }
        }
        if let Some(ref p) = self.options.permission_prompt_tool_name {
            cmd.push("--permission-prompt-tool".to_string());
            cmd.push(p.clone());
        }

        cmd.push("--input-format".to_string());
        cmd.push("stream-json".to_string());

        cmd
    }

    fn build_settings_value(&self) -> Option<String> {
        let has_settings = self.options.settings.is_some();
        let has_sandbox = self.options.sandbox.is_some();

        if !has_settings && !has_sandbox {
            return None;
        }

        // If only settings and no sandbox, pass through as-is (could be file path or JSON)
        if has_settings && !has_sandbox {
            return self.options.settings.clone();
        }

        // If we have sandbox settings, we need to merge into a JSON object
        let mut settings_obj = serde_json::Map::new();
        if let Some(ref s) = self.options.settings {
            let s = s.trim();
            if s.starts_with('{') && s.ends_with('}') {
                // Try to parse as JSON string
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(s) {
                    if let Some(obj) = parsed.as_object() {
                        for (k, v) in obj {
                            settings_obj.insert(k.clone(), v.clone());
                        }
                    }
                }
            } else {
                // It's a file path - read and parse
                let settings_path = std::path::Path::new(s);
                if settings_path.exists() {
                    if let Ok(content) = std::fs::read_to_string(settings_path) {
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&content) {
                            if let Some(obj) = parsed.as_object() {
                                for (k, v) in obj {
                                    settings_obj.insert(k.clone(), v.clone());
                                }
                            }
                        }
                    }
                } else {
                    tracing::warn!("Settings file not found: {}", s);
                }
            }
        }
        if let Some(ref sandbox) = self.options.sandbox {
            let sandbox_val = sandbox_to_json(sandbox);
            settings_obj.insert("sandbox".to_string(), sandbox_val);
        }
        Some(serde_json::to_string(&settings_obj).unwrap_or_default())
    }

    fn resolve_max_thinking_tokens(&self) -> Option<u32> {
        if let Some(ref t) = self.options.thinking {
            match t {
                crate::options::ThinkingConfig::Adaptive => {
                    self.options.max_thinking_tokens.or(Some(32_000))
                }
                crate::options::ThinkingConfig::Enabled { budget_tokens } => Some(*budget_tokens),
                crate::options::ThinkingConfig::Disabled => Some(0),
            }
        } else {
            self.options.max_thinking_tokens
        }
    }
}

fn sandbox_to_json(s: &crate::options::SandboxSettings) -> serde_json::Value {
    let mut m = serde_json::Map::new();
    if let Some(v) = s.enabled {
        m.insert("enabled".to_string(), serde_json::json!(v));
    }
    if let Some(v) = s.auto_allow_bash_if_sandboxed {
        m.insert("autoAllowBashIfSandboxed".to_string(), serde_json::json!(v));
    }
    if let Some(v) = &s.excluded_commands {
        m.insert(
            "excludedCommands".to_string(),
            serde_json::to_value(v).unwrap_or_default(),
        );
    }
    if let Some(v) = s.allow_unsandboxed_commands {
        m.insert("allowUnsandboxedCommands".to_string(), serde_json::json!(v));
    }
    if let Some(ref n) = s.network {
        let mut nm = serde_json::Map::new();
        if let Some(v) = &n.allow_unix_sockets {
            nm.insert(
                "allowUnixSockets".to_string(),
                serde_json::to_value(v).unwrap_or_default(),
            );
        }
        if let Some(v) = n.allow_all_unix_sockets {
            nm.insert("allowAllUnixSockets".to_string(), serde_json::json!(v));
        }
        if let Some(v) = n.allow_local_binding {
            nm.insert("allowLocalBinding".to_string(), serde_json::json!(v));
        }
        if let Some(v) = n.http_proxy_port {
            nm.insert("httpProxyPort".to_string(), serde_json::json!(v));
        }
        if let Some(v) = n.socks_proxy_port {
            nm.insert("socksProxyPort".to_string(), serde_json::json!(v));
        }
        if !nm.is_empty() {
            m.insert("network".to_string(), serde_json::Value::Object(nm));
        }
    }
    if let Some(ref iv) = s.ignore_violations {
        let mut ivm = serde_json::Map::new();
        if let Some(v) = &iv.file {
            ivm.insert(
                "file".to_string(),
                serde_json::to_value(v).unwrap_or_default(),
            );
        }
        if let Some(v) = &iv.network {
            ivm.insert(
                "network".to_string(),
                serde_json::to_value(v).unwrap_or_default(),
            );
        }
        if !ivm.is_empty() {
            m.insert(
                "ignoreViolations".to_string(),
                serde_json::Value::Object(ivm),
            );
        }
    }
    if let Some(v) = s.enable_weaker_nested_sandbox {
        m.insert(
            "enableWeakerNestedSandbox".to_string(),
            serde_json::json!(v),
        );
    }
    serde_json::Value::Object(m)
}

#[async_trait::async_trait]
impl Transport for SubprocessCliTransport {
    async fn connect(&mut self) -> Result<()> {
        if self.process.is_some() {
            return Ok(());
        }

        if std::env::var("CLAUDE_AGENT_SDK_SKIP_VERSION_CHECK").is_err() {
            check_claude_version(&self.cli_path).await;
        }

        let cmd = self.build_command();
        let cmd0 = cmd[0].clone();
        let cmd_rest: Vec<String> = cmd[1..].to_vec();

        let should_pipe_stderr = self.options.stderr.is_some()
            || self.options.extra_args.contains_key("debug-to-stderr");
        let stderr_dest = if should_pipe_stderr {
            Stdio::piped()
        } else {
            Stdio::null()
        };

        let mut child_cmd = Command::new(cmd0);
        child_cmd
            .args(&cmd_rest)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(stderr_dest)
            .env("CLAUDE_CODE_ENTRYPOINT", "sdk-rs")
            .env("CLAUDE_AGENT_SDK_VERSION", env!("CARGO_PKG_VERSION"));

        for (k, v) in &self.options.env {
            child_cmd.env(k, v);
        }
        if self.options.enable_file_checkpointing {
            child_cmd.env("CLAUDE_CODE_ENABLE_SDK_FILE_CHECKPOINTING", "true");
        }
        if let Some(ref cwd) = self.cwd {
            child_cmd.env("PWD", cwd);
            child_cmd.current_dir(cwd);
        }

        // Resolve user option to OS uid (matching Python SDK's `user` param to open_process)
        #[cfg(unix)]
        if let Some(ref username) = self.options.user {
            let user = nix::unistd::User::from_name(username).map_err(|e| {
                Error::Other(format!("Failed to resolve user '{}': {}", username, e))
            })?;
            if let Some(user) = user {
                child_cmd.uid(user.uid.as_raw());
            } else {
                return Err(Error::Other(format!("User '{}' not found", username)));
            }
        }

        let mut process = child_cmd.spawn().map_err(|e| {
            if let Some(ref cwd) = self.cwd {
                if !Path::new(cwd).exists() {
                    return Error::Other(format!("Working directory does not exist: {}", cwd));
                }
            }
            Error::CliNotFound(format!(
                "Claude Code not found at: {} - {}",
                self.cli_path, e
            ))
        })?;

        self.stdin = process.stdin.take();
        self.stdout = process.stdout.take();

        if should_pipe_stderr {
            if let Some(stderr) = process.stderr.take() {
                let stderr_callback = self.options.stderr.clone();
                let stderr_reader = BufReader::new(stderr);
                // Spawn task to read stderr and invoke callback
                tokio::spawn(async move {
                    let mut lines = stderr_reader.lines();
                    while let Ok(Some(line)) = lines.next_line().await {
                        let line_str = line.trim_end();
                        if !line_str.is_empty() {
                            if let Some(ref cb) = stderr_callback {
                                cb(line_str);
                            }
                        }
                    }
                });
            }
        }

        self.process = Some(process);
        self.ready = true;

        Ok(())
    }

    async fn write(&mut self, data: &str) -> Result<()> {
        if !self.ready {
            return Err(Error::Other("Transport not ready for writing".to_string()));
        }
        if let Some(ref mut stdin) = self.stdin {
            use tokio::io::AsyncWriteExt;
            stdin.write_all(data.as_bytes()).await?;
            stdin.flush().await?;
            Ok(())
        } else {
            Err(Error::Other("Stdin not available".to_string()))
        }
    }

    fn read_messages(
        &mut self,
    ) -> std::pin::Pin<Box<dyn futures::Stream<Item = Result<serde_json::Value>> + Send>> {
        let stdout = self.stdout.take();
        let max_buffer_size = self.max_buffer_size;

        let stream = stream! {
            let stdout = match stdout {
                Some(s) => s,
                None => {
                    yield Err(Error::Other("Not connected".to_string()));
                    return;
                }
            };

            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            let mut json_buffer = String::new();

            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        let line = line.trim();
                        if line.is_empty() {
                            continue;
                        }
                        for json_line in line.split('\n') {
                            let json_line = json_line.trim();
                            if json_line.is_empty() {
                                continue;
                            }
                            json_buffer.push_str(json_line);

                            if json_buffer.len() > max_buffer_size {
                                json_buffer.clear();
                                yield Err(Error::Other(format!(
                                    "JSON message exceeded maximum buffer size of {} bytes",
                                    max_buffer_size
                                )));
                                return;
                            }

                            match serde_json::from_str(&json_buffer) {
                                Ok(data) => {
                                    json_buffer.clear();
                                    yield Ok(data);
                                }
                                Err(_) => continue,
                            }
                        }
                    }
                    Ok(None) => break,
                    Err(e) => {
                        yield Err(Error::Connection(e));
                        break;
                    }
                }
            }
        };

        Box::pin(stream)
    }

    async fn close(&mut self) -> Result<()> {
        self.ready = false;
        self.stdin = None;
        self.stdout = None;
        if let Some(mut process) = self.process.take() {
            let _ = process.kill().await;
            let _ = process.wait().await;
        }
        self.exit_error = None;
        Ok(())
    }

    fn is_ready(&self) -> bool {
        self.ready
    }

    async fn end_input(&mut self) -> Result<()> {
        if let Some(stdin) = self.stdin.take() {
            drop(stdin);
        }
        Ok(())
    }
}

fn which_cli() -> Option<String> {
    std::env::var_os("PATH").and_then(|paths| {
        for path in std::env::split_paths(&paths) {
            let full = path.join(if cfg!(target_os = "windows") {
                "claude.exe"
            } else {
                "claude"
            });
            if full.is_file() {
                return Some(full.to_string_lossy().to_string());
            }
        }
        None
    })
}

async fn check_claude_version(cli_path: &str) {
    let result: std::result::Result<(), ()> = async {
        let child = Command::new(cli_path)
            .arg("-v")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| ())?;

        let output = child.wait_with_output().await.map_err(|_| ())?;
        let version_output = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Parse version (e.g., "2.1.0" from "2.1.0" or "2.1.0-beta.1")
        let version_re_match: Option<&str> = version_output
            .split(|c: char| !c.is_ascii_digit() && c != '.')
            .next()
            .filter(|s| s.contains('.'));

        if let Some(version_str) = version_re_match {
            let version_parts: Vec<u32> = version_str
                .split('.')
                .filter_map(|s| s.parse().ok())
                .collect();
            let min_parts: Vec<u32> = MINIMUM_CLAUDE_CODE_VERSION
                .split('.')
                .filter_map(|s| s.parse().ok())
                .collect();

            if version_parts < min_parts {
                let warning = format!(
                    "Warning: Claude Code version {} is unsupported in the Agent SDK. \
                     Minimum required version is {}. Some features may not work correctly.",
                    version_str, MINIMUM_CLAUDE_CODE_VERSION
                );
                tracing::warn!("{}", warning);
                eprintln!("{}", warning);
            }
        }
        Ok(())
    }
    .await;

    let _ = result;
}

// MCP config types for build_command
use crate::options::{McpServerConfig, McpServersConfig};
