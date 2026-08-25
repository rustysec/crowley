# crowley

A [Model Context Protocol](https://modelcontextprotocol.io) (MCP) server that
fetches web content as clean markdown by driving the
[`crwl`](https://github.com/zaferdursun/crwl) CLI.

The server exposes a single `fetch` tool. Every invocation runs
`crwl crawl -o markdown <url>` and returns the extracted markdown to the LLM —
ready to read, summarize, or ask questions about.

## Why crowley?

- **It uses your existing `crwl` install** — no separate fetch engine, no
  duplicate config for proxies, browser profiles, or cache settings.
- **Full `crwl` feature surface**: deep crawling, browser profiles, cache
  bypass, LLM questions (`-q`), JSON/markdown-fit output, and per-page output
  files.
- **Configurable at every level**: built-in defaults → TOML config file → CLI
  flags.
- **Safe process management**: every `crwl` run has a timeout; on expiry the
  child is killed and reaped (no orphaned crawlers), and large responses are
  truncated to a configurable size.

## Requirements

- Rust 1.85+ (uses the `rmcp` 3.x MCP SDK and edition 2024)
- `crwl` on `PATH` (or configured via `crwl_bin`), e.g.:

  ```bash
  crwl --help   # verify
  ```

## Build & run

```bash
cargo build --release
cargo run --release            # serves MCP over stdio
```

### Register with an MCP client

Point your MCP client at the binary. Example for Claude Desktop / generic
stdio clients:

```json
{
  "mcpServers": {
    "crowley": {
      "command": "/absolute/path/to/crowley",
      "args": ["-c", "/absolute/path/to/crowley.toml"]
    }
  }
}
```

### Try it interactively

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2026-07-28","capabilities":{},"clientInfo":{"name":"probe","version":"0"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' \
  '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"fetch","arguments":{"url":"https://example.com"}}}' \
| cargo run --quiet
```

## The `fetch` tool

| Argument       | Meaning                                                                 |
|----------------|-------------------------------------------------------------------------|
| `url`          | **(required)** the page to fetch                                        |
| `output`       | `all` / `json` / `markdown` / `md` / `markdown-fit` / `md-fit`          |
| `profile`      | crwl browser profile (pages behind a login)                             |
| `deep_crawl`   | `bfs` / `dfs` / `best-first` — follow and crawl linked pages            |
| `max_pages`    | cap for deep-crawl mode                                                 |
| `question`     | ask the crwl LLM pipeline a question about the page (returns the answer)|
| `output_file`  | write content to a file; its contents are returned instead              |
| `bypass_cache` | re-fetch from the network, ignoring crwl's cache                        |
| `verbose`      | include crwl crawl progress in the result's stderr appendix             |

Any omitted argument falls back to the server configuration.

## Configuration

Precedence (lowest → highest): **built-in defaults** → **TOML config file**
(`-c/--config`) → **CLI flags**.

### TOML config file

Every key is optional. See [`crowley.toml.example`](crowley.toml.example) for a
fully annotated copy.

```toml
crwl_bin = "crwl"              # path or name of the crwl binary
output_format = "markdown"     # default -o: all|json|markdown|md|markdown-fit|md-fit
profile = "default"            # default -p browser profile
timeout_secs = 60              # kill crwl after this many seconds
max_output_chars = 200000      # truncate tool responses to this many chars
deep_crawl = "bfs"             # default strategy: bfs|dfs|best-first
max_pages = 10                 # default --max-pages
extra_args = []                # extra raw args passed to every `crwl crawl`
server_name = "crowley"        # name reported in the MCP initialize handshake
server_version = "0.1.0"       # version reported in the initialize handshake
verbose = false                # verbose logging + crwl -v by default
```

### CLI flags

```text
-c, --config <PATH>            TOML config file
    --crwl-bin <PATH>          crwl binary override
    --output <FORMAT>          default output format
-p, --profile <NAME>           default browser profile
    --timeout <SECS>           process timeout
    --max-output-chars <N>     response truncation cap
    --deep-crawl <STRATEGY>    default deep-crawl strategy
    --max-pages <N>            default deep-crawl page cap
    --extra-arg <ARG>          extra raw crwl arg (repeatable, appended)
    --server-name <NAME>       MCP server name
    --server-version <VERSION> MCP server version
    --print-config             print the effective config as TOML and exit
-v, --verbose                  verbose logging
```

### Inspect the resolved config

```bash
crowley --print-config
crowley -c crowley.toml --output md-fit --print-config
```

## Development

```bash
cargo test        # config merge/precedence unit tests
cargo build
```

Layout:

- `src/config.rs` — config types, TOML loading, CLI/TOML merge, tests
- `src/crwl.rs` — async `crwl crawl` runner with timeout + kill
- `src/server.rs` — the MCP `fetch` tool and server identity
- `src/main.rs` — CLI, logging, stdio serving, shutdown handling

## License

MIT
