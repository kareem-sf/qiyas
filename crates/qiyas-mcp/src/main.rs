//! `qiyas-mcp`: the small program Claude Desktop and Claude Code start to use Qiyas. It speaks MCP
//! on stdin and stdout and passes the tool calls to the MCP server inside the running app, over
//! HTTP on 127.0.0.1 with the app's token.
//!
//! - `QIYAS_MCP_URL`: the app's MCP address, `http://127.0.0.1:48123/mcp` by default.
//! - `QIYAS_MCP_TOKEN`: the token shown in Qiyas Settings.
//!
//! When the app can't be reached it still starts, and answers each request with a clear message,
//! so the AI app shows why instead of a dead server. It connects again on the next request, so
//! starting Qiyas later is enough. It exits when stdin closes.

use std::process::ExitCode;
use std::sync::Arc;

use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ClientConfig, ContentBlock,
    Implementation, InitializeResult, ListToolsResult, PaginatedRequestParams, ServerCapabilities,
    ServerConfig,
};
use rmcp::service::{RequestContext, RunningService, ServerInitializeError};
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::{ErrorData, Peer, RoleClient, RoleServer, ServerHandler, ServiceError, ServiceExt};
use tokio::sync::Mutex;

const DEFAULT_URL: &str = "http://127.0.0.1:48123/mcp";
const NOT_RUNNING: &str = "Qiyas is not running or MCP is disabled in its settings";
const BAD_TOKEN: &str =
    "Qiyas refused the token. Copy the setup again from Qiyas Settings, MCP server";

type Upstream = RunningService<RoleClient, ClientConfig>;

/// The stdio server: each request goes to the app's MCP server.
struct Bridge {
    url: String,
    token: Option<String>,
    /// The connection to the app; `None` until the first request, and after it broke.
    upstream: Mutex<Option<Upstream>>,
}

impl Bridge {
    /// The app's MCP peer, connecting first when needed. Errs with what to tell the user.
    async fn peer(&self) -> Result<Peer<RoleClient>, &'static str> {
        let mut upstream = self.upstream.lock().await;
        if upstream.as_ref().is_some_and(|u| u.is_transport_closed()) {
            *upstream = None;
        }
        if upstream.is_none() {
            let mut config = StreamableHttpClientTransportConfig::with_uri(self.url.as_str());
            if let Some(token) = &self.token {
                config = config.auth_header(token);
            }
            let transport = StreamableHttpClientTransport::from_config(config);
            match ClientConfig::default().serve(transport).await {
                Ok(service) => *upstream = Some(service),
                Err(e) => {
                    let refused = e.auth_challenge().is_some();
                    eprintln!("qiyas-mcp: could not reach {}: {e}", self.url);
                    return Err(if refused { BAD_TOKEN } else { NOT_RUNNING });
                }
            }
        }
        Ok(upstream.as_ref().expect("connected above").peer().clone())
    }

    /// Maps a failed request: the app's own errors pass through; a broken connection is dropped
    /// so the next request connects again.
    async fn failed(&self, e: ServiceError) -> ErrorData {
        match e {
            ServiceError::McpError(e) => e,
            other => {
                eprintln!("qiyas-mcp: lost the connection to {}: {other}", self.url);
                *self.upstream.lock().await = None;
                ErrorData::internal_error(NOT_RUNNING, None)
            }
        }
    }
}

impl ServerHandler for Bridge {
    fn get_info(&self) -> ServerConfig {
        InitializeResult::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("qiyas", env!("CARGO_PKG_VERSION")))
    }

    async fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let peer = self
            .peer()
            .await
            .map_err(|m| ErrorData::internal_error(m, None))?;
        match peer.list_tools(request).await {
            Ok(tools) => Ok(tools),
            Err(e) => Err(self.failed(e).await),
        }
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let peer = match self.peer().await {
            Ok(peer) => peer,
            Err(m) => return Ok(CallToolResult::error(vec![ContentBlock::text(m)]).into()),
        };
        match peer.call_tool_once(request).await {
            Ok(response) => Ok(response),
            Err(e) => {
                let e = self.failed(e).await;
                Ok(CallToolResult::error(vec![ContentBlock::text(e.message)]).into())
            }
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    // Fails only when a provider is already installed.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let bridge = Bridge {
        url: std::env::var("QIYAS_MCP_URL")
            .ok()
            .filter(|u| !u.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_URL.into()),
        token: std::env::var("QIYAS_MCP_TOKEN")
            .ok()
            .filter(|t| !t.trim().is_empty()),
        upstream: Mutex::new(None),
    };
    let bridge = Arc::new(bridge);
    let service = match bridge.clone().serve(rmcp::transport::stdio()).await {
        Ok(service) => service,
        // stdin closed before the AI app said hello.
        Err(ServerInitializeError::ConnectionClosed(_))
        | Err(ServerInitializeError::ExpectedInitializeRequest(None)) => return ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("qiyas-mcp: the MCP handshake on stdin failed: {e}");
            return ExitCode::FAILURE;
        }
    };
    // Ends when stdin closes.
    let _ = service.waiting().await;
    if let Some(upstream) = bridge.upstream.lock().await.take() {
        let _ = upstream.cancel().await;
    }
    ExitCode::SUCCESS
}
