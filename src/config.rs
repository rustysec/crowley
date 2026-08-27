//! Configuration for the crowley MCP server.
//!
//! Settings can be provided through a TOML config file, CLI flags, or both.
//! Precedence (lowest to highest): built-in defaults < config file < CLI flags.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

/// Output formats accepted by `crwl crawl -o`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Deserialize, Serialize)]
pub enum OutputFormat {
    #[value(name = "all")]
    #[serde(rename = "all")]
    All,
    #[value(name = "json")]
    #[serde(rename = "json")]
    Json,
    #[value(name = "markdown")]
    #[serde(rename = "markdown")]
    Markdown,
    #[value(name = "md")]
    #[serde(rename = "md")]
    Md,
    #[value(name = "markdown-fit")]
    #[serde(rename = "markdown-fit")]
    MarkdownFit,
    #[value(name = "md-fit")]
    #[serde(rename = "md-fit")]
    MdFit,
}

impl OutputFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            OutputFormat::All => "all",
            OutputFormat::Json => "json",
            OutputFormat::Markdown => "markdown",
            OutputFormat::Md => "md",
            OutputFormat::MarkdownFit => "markdown-fit",
            OutputFormat::MdFit => "md-fit",
        }
    }
}

/// Deep-crawl strategies accepted by `crwl crawl --deep-crawl`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Deserialize, Serialize)]
pub enum DeepCrawlStrategy {
    #[value(name = "bfs")]
    #[serde(rename = "bfs")]
    Bfs,
    #[value(name = "dfs")]
    #[serde(rename = "dfs")]
    Dfs,
    #[value(name = "best-first")]
    #[serde(rename = "best-first")]
    BestFirst,
}

impl DeepCrawlStrategy {
    pub fn as_str(&self) -> &'static str {
        match self {
            DeepCrawlStrategy::Bfs => "bfs",
            DeepCrawlStrategy::Dfs => "dfs",
            DeepCrawlStrategy::BestFirst => "best-first",
        }
    }
}

/// Transports the MCP server can serve over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Deserialize, Serialize)]
pub enum TransportMode {
    /// MCP over the process's stdin/stdout (default for local hosts).
    #[value(name = "stdio")]
    #[serde(rename = "stdio")]
    Stdio,
    /// MCP Streamable HTTP — listen on a TCP port for remote clients.
    #[value(name = "http")]
    #[serde(rename = "http")]
    Http,
}

impl TransportMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            TransportMode::Stdio => "stdio",
            TransportMode::Http => "http",
        }
    }
}

fn default_crwl_bin() -> String {
    "crwl".to_string()
}

fn default_transport() -> TransportMode {
    TransportMode::Stdio
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    4321
}

fn default_http_path() -> String {
    "/mcp".to_string()
}

fn default_allowed_hosts() -> Vec<String> {
    vec!["localhost".into(), "127.0.0.1".into(), "::1".into()]
}

fn default_output_format() -> OutputFormat {
    OutputFormat::Markdown
}

fn default_timeout_secs() -> u64 {
    60
}

fn default_max_output_chars() -> usize {
    200_000
}

fn default_server_name() -> String {
    "crowley".to_string()
}

fn default_server_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Fully-resolved server configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    /// Path to the `crwl` binary (or a name resolvable on `PATH`).
    #[serde(default = "default_crwl_bin")]
    pub crwl_bin: String,
    /// Default `-o` output format passed to `crwl crawl`.
    #[serde(default = "default_output_format")]
    pub output_format: OutputFormat,
    /// Default browser profile (`crwl -p <profile>`).
    #[serde(default)]
    pub profile: Option<String>,
    /// Path to a browser config file (`crwl -B <path>`, YAML/JSON).
    #[serde(default)]
    pub browser_config: Option<String>,
    /// Path to a crawler config file (`crwl -C <path>`, YAML/JSON).
    #[serde(default)]
    pub crawler_config: Option<String>,
    /// Timeout (seconds) before a `crwl` invocation is killed.
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    /// Cap on characters returned in a tool result (overflow is truncated).
    #[serde(default = "default_max_output_chars")]
    pub max_output_chars: usize,
    /// Default deep-crawl strategy (`--deep-crawl <strategy>`).
    #[serde(default)]
    pub deep_crawl: Option<DeepCrawlStrategy>,
    /// Default max pages for deep crawls (`--max-pages <n>`).
    #[serde(default)]
    pub max_pages: Option<u32>,
    /// Extra raw arguments forwarded to every `crwl crawl` invocation.
    #[serde(default)]
    pub extra_args: Vec<String>,
    /// Server name reported in the MCP `initialize` handshake.
    #[serde(default = "default_server_name")]
    pub server_name: String,
    /// Server version reported in the MCP `initialize` handshake.
    #[serde(default = "default_server_version")]
    pub server_version: String,
    /// Emit verbose logging from this server.
    #[serde(default)]
    pub verbose: bool,
    /// MCP transport to serve over: `stdio` (default) or `http`.
    #[serde(default = "default_transport")]
    pub transport: TransportMode,
    /// Bind address for `http` transport.
    #[serde(default = "default_host")]
    pub host: String,
    /// Bind port for `http` transport. `0` binds an ephemeral port (the
    /// actual port is logged at startup).
    #[serde(default = "default_port")]
    pub port: u16,
    /// URL path the MCP Streamable HTTP endpoint is mounted at.
    #[serde(default = "default_http_path")]
    pub http_path: String,
    /// Allowed `Host` header values for the `http` transport (prevents DNS
    /// rebinding). Add the hostnames clients will use, e.g. `0.0.0.0` or a
    /// reverse-proxy hostname.
    #[serde(default = "default_allowed_hosts")]
    pub allowed_hosts: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            crwl_bin: default_crwl_bin(),
            output_format: default_output_format(),
            profile: None,
            browser_config: None,
            crawler_config: None,
            timeout_secs: default_timeout_secs(),
            max_output_chars: default_max_output_chars(),
            deep_crawl: None,
            max_pages: None,
            extra_args: Vec::new(),
            server_name: default_server_name(),
            server_version: default_server_version(),
            verbose: false,
            transport: default_transport(),
            host: default_host(),
            port: default_port(),
            http_path: default_http_path(),
            allowed_hosts: default_allowed_hosts(),
        }
    }
}

/// The TOML config file layout. All fields are optional.
///
/// ```toml
/// crwl_bin = "crwl"
/// output_format = "markdown"
/// profile = "default"
/// browser_config = "/path/to/browser.yaml"
/// crawler_config = "/path/to/crawler.yaml"
/// timeout_secs = 60
/// max_output_chars = 200000
/// deep_crawl = "bfs"
/// max_pages = 10
/// extra_args = ["--bypass-cache"]
/// server_name = "crowley"
/// server_version = "0.1.0"
/// verbose = false
/// ```
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigFile {
    pub crwl_bin: Option<String>,
    pub output_format: Option<OutputFormat>,
    pub profile: Option<String>,
    pub browser_config: Option<String>,
    pub crawler_config: Option<String>,
    pub timeout_secs: Option<u64>,
    pub max_output_chars: Option<usize>,
    pub deep_crawl: Option<DeepCrawlStrategy>,
    pub max_pages: Option<u32>,
    pub extra_args: Option<Vec<String>>,
    pub server_name: Option<String>,
    pub server_version: Option<String>,
    pub verbose: Option<bool>,
    pub transport: Option<TransportMode>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub http_path: Option<String>,
    pub allowed_hosts: Option<Vec<String>>,
}

/// Command-line options. Each flag, when present, overrides the config file.
#[derive(Debug, Clone, clap::Parser)]
#[command(
    name = "crowley",
    version,
    about = "MCP server that fetches web content as markdown via the crwl CLI",
    long_about = "A Model Context Protocol (MCP) server exposing a `fetch` tool that runs the\n\
                  `crwl` CLI to read web pages as clean markdown.\n\n\
                  Configuration is merged from (lowest to highest precedence):\n\
                  built-in defaults, a TOML config file (--config), and CLI flags."
)]
pub struct Cli {
    /// Path to a TOML configuration file.
    #[arg(short = 'c', long = "config", value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Path to the `crwl` binary (default: "crwl" from PATH).
    #[arg(long = "crwl-bin", value_name = "PATH")]
    pub crwl_bin: Option<String>,

    /// Default output format for fetched content.
    #[arg(long = "output", value_name = "FORMAT", value_enum)]
    pub output_format: Option<OutputFormat>,

    /// Default browser profile used by crwl.
    #[arg(short = 'p', long = "profile", value_name = "NAME")]
    pub profile: Option<String>,

    /// Path to a crwl browser config file (YAML/JSON) passed via -B.
    #[arg(long = "browser-config", value_name = "PATH")]
    pub browser_config: Option<String>,

    /// Path to a crwl crawler config file (YAML/JSON) passed via -C.
    #[arg(long = "crawler-config", value_name = "PATH")]
    pub crawler_config: Option<String>,

    /// Kill crwl invocations after this many seconds.
    #[arg(long = "timeout", value_name = "SECS")]
    pub timeout_secs: Option<u64>,

    /// Maximum characters returned per tool call (overflow is truncated).
    #[arg(long = "max-output-chars", value_name = "N")]
    pub max_output_chars: Option<usize>,

    /// Default deep-crawl strategy (bfs | dfs | best-first).
    #[arg(long = "deep-crawl", value_name = "STRATEGY", value_enum)]
    pub deep_crawl: Option<DeepCrawlStrategy>,

    /// Default max pages for deep crawls.
    #[arg(long = "max-pages", value_name = "N")]
    pub max_pages: Option<u32>,

    /// Extra raw argument forwarded to every `crwl crawl` call (repeatable).
    #[arg(long = "extra-arg", value_name = "ARG")]
    pub extra_args: Vec<String>,

    /// Server name reported in the MCP initialize handshake.
    #[arg(long = "server-name", value_name = "NAME")]
    pub server_name: Option<String>,

    /// Server version reported in the MCP initialize handshake.
    #[arg(long = "server-version", value_name = "VERSION")]
    pub server_version: Option<String>,

    /// Print the effective configuration as TOML and exit.
    #[arg(long = "print-config")]
    pub print_config: bool,

    /// Emit verbose logging.
    #[arg(long = "verbose", short = 'v')]
    pub verbose: bool,
    /// MCP transport: stdio (default) or http.
    #[arg(long = "transport", value_name = "MODE", value_enum)]
    pub transport: Option<TransportMode>,
    /// Bind address for the http transport.
    #[arg(long = "host", value_name = "HOST")]
    pub host: Option<String>,
    /// Bind port for the http transport (0 = ephemeral).
    #[arg(long = "port", value_name = "PORT")]
    pub port: Option<u16>,
    /// URL path for the http transport's MCP endpoint.
    #[arg(long = "http-path", value_name = "PATH")]
    pub http_path: Option<String>,
    /// Extra allowed Host header for the http transport (repeatable).
    #[arg(long = "allowed-host", value_name = "HOST")]
    pub allowed_hosts: Vec<String>,
}

impl Config {
    /// Load configuration from an optional TOML file, overlaying it on defaults.
    pub fn load(config_path: Option<&std::path::Path>) -> Result<Self> {
        let mut config = Config::default();
        if let Some(path) = config_path {
            let text = std::fs::read_to_string(path)
                .with_context(|| format!("failed to read config file `{}`", path.display()))?;
            let file: ConfigFile = toml::from_str(&text).with_context(|| {
                format!("failed to parse config file `{}` as TOML", path.display())
            })?;
            config.apply_file(file);
        }
        Ok(config)
    }

    /// Overlay CLI flags on top of the current configuration.
    pub fn apply_cli(&mut self, cli: &Cli) {
        if let Some(bin) = &cli.crwl_bin {
            self.crwl_bin = bin.clone();
        }
        if let Some(format) = cli.output_format {
            self.output_format = format;
        }
        if cli.profile.is_some() {
            self.profile = cli.profile.clone();
        }
        if cli.browser_config.is_some() {
            self.browser_config = cli.browser_config.clone();
        }
        if cli.crawler_config.is_some() {
            self.crawler_config = cli.crawler_config.clone();
        }
        if let Some(timeout) = cli.timeout_secs {
            self.timeout_secs = timeout;
        }
        if let Some(max) = cli.max_output_chars {
            self.max_output_chars = max;
        }
        if cli.deep_crawl.is_some() {
            self.deep_crawl = cli.deep_crawl;
        }
        if cli.max_pages.is_some() {
            self.max_pages = cli.max_pages;
        }
        if !cli.extra_args.is_empty() {
            self.extra_args.extend(cli.extra_args.clone());
        }
        if let Some(name) = &cli.server_name {
            self.server_name = name.clone();
        }
        if let Some(version) = &cli.server_version {
            self.server_version = version.clone();
        }
        if cli.verbose {
            self.verbose = true;
        }
        if let Some(transport) = cli.transport {
            self.transport = transport;
        }
        if let Some(host) = &cli.host {
            self.host = host.clone();
        }
        if let Some(port) = cli.port {
            self.port = port;
        }
        if let Some(path) = &cli.http_path {
            self.http_path = path.clone();
        }
        if !cli.allowed_hosts.is_empty() {
            self.allowed_hosts.extend(cli.allowed_hosts.clone());
        }
    }

    fn apply_file(&mut self, file: ConfigFile) {
        if let Some(bin) = file.crwl_bin {
            self.crwl_bin = bin;
        }
        if let Some(format) = file.output_format {
            self.output_format = format;
        }
        if let Some(profile) = file.profile {
            self.profile = Some(profile);
        }
        if let Some(path) = file.browser_config {
            self.browser_config = Some(path);
        }
        if let Some(path) = file.crawler_config {
            self.crawler_config = Some(path);
        }
        if let Some(timeout) = file.timeout_secs {
            self.timeout_secs = timeout;
        }
        if let Some(max) = file.max_output_chars {
            self.max_output_chars = max;
        }
        if let Some(strategy) = file.deep_crawl {
            self.deep_crawl = Some(strategy);
        }
        if let Some(pages) = file.max_pages {
            self.max_pages = Some(pages);
        }
        if let Some(args) = file.extra_args {
            self.extra_args = args;
        }
        if let Some(name) = file.server_name {
            self.server_name = name;
        }
        if let Some(version) = file.server_version {
            self.server_version = version;
        }
        if let Some(verbose) = file.verbose {
            self.verbose = verbose;
        }
        if let Some(transport) = file.transport {
            self.transport = transport;
        }
        if let Some(host) = file.host {
            self.host = host;
        }
        if let Some(port) = file.port {
            self.port = port;
        }
        if let Some(path) = file.http_path {
            self.http_path = path;
        }
        if let Some(hosts) = file.allowed_hosts {
            self.allowed_hosts = hosts;
        }
    }

    /// Render the effective configuration as pretty TOML.
    pub fn to_toml(&self) -> Result<String> {
        toml::to_string_pretty(self).context("failed to serialize configuration")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane() {
        let config = Config::default();
        assert_eq!(config.crwl_bin, "crwl");
        assert_eq!(config.output_format, OutputFormat::Markdown);
        assert_eq!(config.timeout_secs, 60);
        assert!(config.max_output_chars > 0);
        assert_eq!(config.server_name, "crowley");
        assert_eq!(config.transport, TransportMode::Stdio);
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 4321);
        assert_eq!(config.http_path, "/mcp");
    }

    #[test]
    fn config_file_overrides_defaults() {
        let text = r#"
            crwl_bin = "/opt/bin/crwl"
            output_format = "md-fit"
            profile = "work"
            browser_config = "/etc/crwl/browser.yaml"
            crawler_config = "/etc/crwl/crawler.yaml"
            timeout_secs = 120
            max_output_chars = 1000
            deep_crawl = "best-first"
            max_pages = 5
            extra_args = ["-bc"]
        "#;
        let file: ConfigFile =
            toml::from_str(text).expect("test fixture must be valid TOML");
        let mut config = Config::default();
        config.apply_file(file);
        assert_eq!(config.crwl_bin, "/opt/bin/crwl");
        assert_eq!(config.output_format, OutputFormat::MdFit);
        assert_eq!(config.profile.as_deref(), Some("work"));
        assert_eq!(config.browser_config.as_deref(), Some("/etc/crwl/browser.yaml"));
        assert_eq!(config.crawler_config.as_deref(), Some("/etc/crwl/crawler.yaml"));
        assert_eq!(config.timeout_secs, 120);
        assert_eq!(config.max_output_chars, 1000);
        assert_eq!(config.deep_crawl, Some(DeepCrawlStrategy::BestFirst));
        assert_eq!(config.max_pages, Some(5));
        assert_eq!(config.extra_args, vec!["-bc"]);
    }

    #[test]
    fn cli_overrides_config_file() {
        let mut config = Config::default();
        config.timeout_secs = 30;
        let cli = Cli {
            config: None,
            crwl_bin: Some("crwl2".into()),
            output_format: None,
            profile: None,
            browser_config: Some("browser.yaml".into()),
            crawler_config: None,
            timeout_secs: Some(90),
            max_output_chars: None,
            deep_crawl: None,
            max_pages: None,
            extra_args: vec!["-v".into()],
            server_name: None,
            server_version: None,
            print_config: false,
            verbose: true,
            transport: Some(TransportMode::Http),
            host: Some("0.0.0.0".into()),
            port: Some(9000),
            http_path: Some("/api/mcp".into()),
            allowed_hosts: vec!["example.com".into()],
        };
        config.apply_cli(&cli);
        assert_eq!(config.crwl_bin, "crwl2");
        assert_eq!(config.timeout_secs, 90);
        assert_eq!(config.browser_config.as_deref(), Some("browser.yaml"));
        assert_eq!(config.crawler_config, None);
        assert_eq!(config.extra_args, vec!["-v"]);
        assert!(config.verbose);
        assert_eq!(config.output_format, OutputFormat::Markdown);
        assert_eq!(config.transport, TransportMode::Http);
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 9000);
        assert_eq!(config.http_path, "/api/mcp");
        // --allowed-host appends to the default loopback allowlist.
        assert_eq!(
            config.allowed_hosts,
            vec!["localhost", "127.0.0.1", "::1", "example.com"]
        );
    }

    #[test]
    fn round_trip_toml() {
        let config = Config::default();
        let text = config.to_toml().expect("serialization must not fail");
        let parsed: Config =
            toml::from_str(&text).expect("serialized config must re-parse");
        assert_eq!(parsed.crwl_bin, config.crwl_bin);
        assert_eq!(parsed.output_format, config.output_format);
    }
}
