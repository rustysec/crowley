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
    /// `-B` browser config file (YAML/JSON).
    pub browser_config: Option<PathBuf>,
    /// `-C` crawler config file (YAML/JSON).
    pub crawler_config: Option<PathBuf>,
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
        if let Some(path) = &request.browser_config {
            command.arg("-B").arg(path);
        }
        if let Some(path) = &request.crawler_config {
            command.arg("-C").arg(path);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Write an executable fake `crwl` that dumps its argv to a fixed file
    /// (one arg per line) and exits 0.
    fn fake_crwl(argv_file: &std::path::Path) -> std::path::PathBuf {
        let dir = argv_file.parent().expect("argv file needs a parent dir");
        let bin = dir.join("crwl");
        std::fs::write(
            &bin,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > {}\n",
                argv_file.display()
            ),
        )
        .expect("write fake crwl");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&bin).expect("stat fake crwl").permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&bin, perms).expect("chmod fake crwl");
        }
        bin
    }

    #[tokio::test]
    async fn browser_and_crawler_config_flags_reach_crwl() {
        let dir = std::env::temp_dir().join(format!("crowley-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let argv_file = dir.join("argv.txt");
        let bin = fake_crwl(&argv_file);
        let browser = dir.join("browser.yaml");
        let crawler = dir.join("crawler.yaml");
        std::fs::write(&browser, "headless: true\n").expect("write browser config");
        std::fs::write(&crawler, "max_depth: 2\n").expect("write crawler config");

        let runner = CrwlRunner::new(bin.display().to_string(), Duration::from_secs(10));
        let request = FetchRequest {
            url: "https://example.com".into(),
            output: Some("markdown".into()),
            profile: None,
            browser_config: Some(browser.clone()),
            crawler_config: Some(crawler.clone()),
            deep_crawl: None,
            max_pages: None,
            question: None,
            output_file: None,
            bypass_cache: false,
            verbose: false,
            extra_args: vec![],
        };

        let result = runner.fetch(&request).await.expect("fake crwl must run");
        assert_eq!(result.status, Some(0), "fake crwl should exit 0");

        let argv = std::fs::read_to_string(&argv_file).expect("read recorded argv");
        assert!(argv.contains("crawl"), "crawl subcommand first: {argv}");
        assert!(argv.contains(&format!("-B\n{}", browser.display())));
        assert!(argv.contains(&format!("-C\n{}", crawler.display())));
        assert!(argv.trim_end().ends_with("https://example.com"));

        std::fs::remove_dir_all(&dir).expect("clean up temp dir");
    }
}
