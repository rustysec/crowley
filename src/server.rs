//! The MCP server: exposes a `fetch` tool that shells out to `crwl`.

use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::crwl::{CrwlRunner, FetchOutput, FetchRequest, fetch_error};

/// Output formats advertised in the tool schema. Mirrors `crwl -o`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum OutputChoice {
    /// Everything the crawler extracted.
    All,
    /// Structured JSON extraction.
    Json,
    /// Clean markdown (default).
    Markdown,
    /// `markdown` alias.
    Md,
    /// Markdown fitted to the page's main content.
    MarkdownFit,
    /// `markdown-fit` alias.
    MdFit,
}

impl OutputChoice {
    pub fn as_str(&self) -> &'static str {
        match self {
            OutputChoice::All => "all",
            OutputChoice::Json => "json",
            OutputChoice::Markdown => "markdown",
            OutputChoice::Md => "md",
            OutputChoice::MarkdownFit => "markdown-fit",
            OutputChoice::MdFit => "md-fit",
        }
    }
}

/// Deep-crawl strategies advertised in the tool schema. Mirrors
/// `crwl --deep-crawl`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum DeepCrawlChoice {
    /// Breadth-first traversal of linked pages.
    Bfs,
    /// Depth-first traversal of linked pages.
    Dfs,
    /// Follow the most promising links first.
    BestFirst,
}

impl DeepCrawlChoice {
    pub fn as_str(&self) -> &'static str {
        match self {
            DeepCrawlChoice::Bfs => "bfs",
            DeepCrawlChoice::Dfs => "dfs",
            DeepCrawlChoice::BestFirst => "best-first",
        }
    }
}

/// Arguments accepted by the `fetch` tool.
///
/// Every field is optional except `url`.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct FetchParams {
    /// The URL of the web page to fetch.
    pub url: String,
    /// Output format. Defaults to the server's configured output format
    /// (markdown).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<OutputChoice>,
    /// Browser profile to use (e.g. for pages behind a login).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    /// Path to a crwl browser config file (YAML/JSON), passed via -B.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser_config: Option<String>,
    /// Path to a crwl crawler config file (YAML/JSON), passed via -C.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crawler_config: Option<String>,
    /// Deep-crawl strategy. Follows and crawls linked pages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deep_crawl: Option<DeepCrawlChoice>,
    /// Maximum number of pages to crawl in deep-crawl mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_pages: Option<u32>,
    /// Ask the crwl LLM pipeline a question about the page content; the
    /// answer is returned instead of the raw markdown.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub question: Option<String>,
    /// Write the fetched content to this file instead of returning it inline.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_file: Option<String>,
    /// Bypass the crawl cache and re-fetch from the network.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bypass_cache: Option<bool>,
    /// Emit verbose crawl progress (included as diagnostics on the result).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verbose: Option<bool>,
}

/// The crowley MCP server.
#[derive(Debug)]
pub struct CrowleyServer {
    config: Arc<Config>,
    crwl: CrwlRunner,
    tool_router: ToolRouter<Self>,
}

impl CrowleyServer {
    /// Build a server with the resolved configuration.
    pub fn new(config: Arc<Config>) -> Self {
        let crwl = CrwlRunner::new(
            config.crwl_bin.clone(),
            std::time::Duration::from_secs(config.timeout_secs.max(1)),
        );
        Self {
            config,
            crwl,
            tool_router: Self::tool_router(),
        }
    }

    /// Execute one `crwl crawl` for a tool call.
    async fn run_fetch(&self, params: FetchParams) -> Result<String, String> {
        let output = params
            .output
            .map(|choice| choice.as_str().to_string())
            .unwrap_or_else(|| self.config.output_format.as_str().to_string());
        let deep_crawl = params
            .deep_crawl
            .map(|choice| choice.as_str().to_string())
            .or_else(|| self.config.deep_crawl.map(|strategy| strategy.as_str().to_string()));

        let request = FetchRequest {
            url: params.url.clone(),
            output: Some(output),
            profile: params.profile.or_else(|| self.config.profile.clone()),
            browser_config: params
                .browser_config
                .or_else(|| self.config.browser_config.clone())
                .map(Into::into),
            crawler_config: params
                .crawler_config
                .or_else(|| self.config.crawler_config.clone())
                .map(Into::into),
            deep_crawl,
            max_pages: params.max_pages.or(self.config.max_pages),
            question: params.question,
            output_file: params.output_file.map(Into::into),
            bypass_cache: params.bypass_cache.unwrap_or(false),
            verbose: params.verbose.unwrap_or(self.config.verbose),
            extra_args: self.config.extra_args.clone(),
        };

        tracing::debug!(url = %request.url, "running crwl crawl");
        let started = std::time::Instant::now();
        let output = self
            .crwl
            .fetch(&request)
            .await
            .map_err(|err| err.to_string())?;
        let elapsed = started.elapsed();
        tracing::debug!(url = %request.url, elapsed_ms = elapsed.as_millis(), exit = ?output.status, "crwl crawl finished");

        let success = output.status == Some(0) && !output.timed_out;
        if !success {
            return Err(fetch_error(&output, &request.url).to_string());
        }

        let content = if let Some(path) = &request.output_file {
            match std::fs::read_to_string(path) {
                Ok(text) => text,
                Err(err) => {
                    return Err(format!(
                        "crwl succeeded but its output file `{}` could not be read: {err}",
                        path.display()
                    ))
                }
            }
        } else {
            output.stdout.clone()
        };

        self.render_result(content, output)
    }

    /// Apply truncation and attach stderr diagnostics to the fetched content.
    fn render_result(&self, content: String, output: FetchOutput) -> Result<String, String> {
        let original_len = content.len();
        let mut result = content;

        let stderr = output.stderr.trim();
        if !stderr.is_empty() {
            result.push_str("\n\n[crwl stderr]\n");
            result.push_str(truncate_at_char_boundary(stderr, 2000));
        }

        let max = self.config.max_output_chars.max(1);
        if result.len() > max {
            let truncated = truncate_at_char_boundary(&result, max).to_string();
            result = format!(
                "{truncated}\n\n[truncated: showing {max} of {original_len} characters]"
            );
        }

        Ok(result)
    }
}

/// Truncate `s` to at most `max` bytes without splitting a UTF-8 character.
fn truncate_at_char_boundary(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Tool registration. Each `#[tool]` method becomes an MCP tool routed by the
/// generated [`ToolRouter`].
#[tool_router(router = tool_router)]
impl CrowleyServer {
    /// Fetch a web page and return its content as markdown.
    ///
    /// Runs `crwl crawl -o markdown <url>` and returns the extracted content,
    /// ready to be read or summarized. Pass `output_file` for very large
    /// pages (contents are returned from the file), `profile` for pages
    /// behind a login, `browser_config`/`crawler_config` for YAML/JSON
    /// config files, `deep_crawl`/`max_pages` to follow links, or
    /// `question` to get an LLM-generated answer about the page instead.
    #[tool]
    async fn fetch(&self, params: Parameters<FetchParams>) -> Result<String, String> {
        self.run_fetch(params.0).await
    }
}

/// Serve handler. `get_info` is hand-written so the server identity comes from
/// the runtime configuration; everything else is generated.
#[tool_handler(router = self.tool_router)]
impl ServerHandler for CrowleyServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                self.config.server_name.clone(),
                self.config.server_version.clone(),
            ))
            .with_instructions(format!(
                "Fetch web content as markdown with the `{}` tool, backed by the crwl CLI. \
                 Set defaults via a TOML config file or CLI flags.",
                self.config.tool_name()
            ))
    }
}

impl Config {
    /// The name under which the fetch tool is exposed (fixed at compile time).
    pub fn tool_name(&self) -> &'static str {
        "fetch"
    }
}
