//! Thin async wrapper around the `crwl` CLI.
//!
//! Builds a `crwl crawl` command from a [`FetchRequest`], runs it with a
//! timeout, and returns its captured stdout/stderr. On timeout the child
//! process is killed and reaped.

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use tokio::io::AsyncReadExt;
use tokio::process::Command;

/// A single `crwl crawl` invocation.
#[derive(Debug, Clone)]
pub struct FetchRequest {
    /// The URL to fetch. Required.
    pub url: String,
    /// `-o` output format (`all`, `json`, `markdown`, `md`, `markdown-fit`, `md-fit`).
    pub output: Option<String>,
    /// `-p` browser profile name.
    pub profile: Option<String>,
    /// `--deep-crawl` strategy (`bfs`, `dfs`, `best-first`).
    pub deep_crawl: Option<String>,
    /// `--max-pages` limit for deep crawls.
    pub max_pages: Option<u32>,
    /// `-q` LLM question about the page content.
    pub question: Option<String>,
    /// `-O` output file path. When set, the fetched content is written there
    /// instead of stdout.
    pub output_file: Option<PathBuf>,
    /// `-bc` bypass the crawl cache.
    pub bypass_cache: bool,
    /// `-v` verbose crawl progress (goes to stderr).
    pub verbose: bool,
    /// Extra raw arguments forwarded verbatim to `crwl crawl`.
    pub extra_args: Vec<String>,
}

/// Captured output of a `crwl crawl` invocation.
#[derive(Debug)]
pub struct FetchOutput {
    /// Everything the process wrote to stdout.
    pub stdout: String,
    /// Everything the process wrote to stderr.
    pub stderr: String,
    /// Exit status code, if the process exited normally.
    pub status: Option<i32>,
    /// True if the process was killed after exceeding the timeout.
    pub timed_out: bool,
}

/// Runs the `crwl` binary.
#[derive(Debug, Clone)]
pub struct CrwlRunner {
    bin: String,
    timeout: Duration,
}

impl CrwlRunner {
    pub fn new(bin: String, timeout: Duration) -> Self {
        Self { bin, timeout }
    }

    /// Run `crwl crawl` for the given request, capturing output.
    ///
    /// Always returns a [`FetchOutput`] for a successfully spawned process —
    /// even when the exit code is non-zero — so the caller can surface useful
    /// stderr diagnostics. Only spawn/IO failures produce `Err`.
    pub async fn fetch(&self, request: &FetchRequest) -> Result<FetchOutput> {
        let mut command = Command::new(&self.bin);
        command.arg("crawl");

        // User-supplied extra args first so they can be overridden by the
        // explicit flags below.
        command.args(&request.extra_args);

        command
            .arg("-o")
            .arg(request.output.as_deref().unwrap_or("markdown"));

        if let Some(profile) = &request.profile {
            command.arg("-p").arg(profile);
        }
        if let Some(strategy) = &request.deep_crawl {
            command.arg("--deep-crawl").arg(strategy);
        }
        if let Some(max_pages) = request.max_pages {
            command.arg("--max-pages").arg(max_pages.to_string());
        }
        if let Some(question) = &request.question {
            // -q expects a quoted query string; pass it as a single argv entry.
            command.arg("-q").arg(question);
        }
        if let Some(path) = &request.output_file {
            command.arg("-O").arg(path);
        }
        if request.bypass_cache {
            command.arg("-bc");
        }
        if request.verbose {
            command.arg("-v");
        }
        command.arg(&request.url);

        command.stdout(Stdio::piped()).stderr(Stdio::piped());

        let mut child = command
            .spawn()
            .with_context(|| spawn_hint(&self.bin, &request.url))?;

        let mut stdout = child.stdout.take().expect("stdout should be piped");
        let mut stderr = child.stderr.take().expect("stderr should be piped");
        let stdout_task = tokio::spawn(async move {
            let mut buf = Vec::new();
            stdout.read_to_end(&mut buf).await?;
            Ok::<_, std::io::Error>(buf)
        });
        let stderr_task = tokio::spawn(async move {
            let mut buf = Vec::new();
            stderr.read_to_end(&mut buf).await?;
            Ok::<_, std::io::Error>(buf)
        });

        let waited = tokio::time::timeout(self.timeout, child.wait()).await;

        let (timed_out, status) = match waited {
            Ok(Ok(status)) => (false, status.code()),
            Ok(Err(err)) => {
                let _ = child.kill().await;
                return Err(err).with_context(|| format!("failed to wait on `{}`", self.bin));
            }
            Err(_elapsed) => {
                // Timeout: kill the process and reap it so it can't linger.
                let _ = child.kill().await;
                let _ = child.wait().await;
                (true, None)
            }
        };

        let stdout = stdout_task
            .await
            .context("stdout reader task panicked")?
            .with_context(|| format!("failed to read stdout from `{}`", self.bin))?;
        let stderr = stderr_task
            .await
            .context("stderr reader task panicked")?
            .with_context(|| format!("failed to read stderr from `{}`", self.bin))?;

        Ok(FetchOutput {
            stdout: String::from_utf8_lossy(&stdout).into_owned(),
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
            status,
            timed_out,
        })
    }
}

fn spawn_hint(bin: &str, url: &str) -> String {
    format!(
        "failed to spawn `{bin}` while fetching `{url}` — is the crwl binary installed and on PATH?"
    )
}

/// Build a human-readable error describing a failed `crwl crawl` run.
pub fn fetch_error(exit: &FetchOutput, url: &str) -> anyhow::Error {
    let stderr = exit.stderr.trim();
    let kind = if exit.timed_out {
        "timed out".to_string()
    } else {
        match exit.status {
            Some(code) => format!("exited with status {code}"),
            None => "was terminated".to_string(),
        }
    };
    let detail = if stderr.is_empty() {
        String::new()
    } else {
        let len = stderr.len().min(2000);
        format!(": {}", &stderr[..len])
    };
    let message = format!("crwl {kind} while fetching `{url}`{detail}").trim().to_string();
    anyhow!(message)
}
