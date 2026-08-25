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

### stdio (default)

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

### HTTP (Streamable HTTP)

Start crowley in HTTP mode and point remote clients at the endpoint:

```bash
crowley --transport http --port 4321           # http://127.0.0.1:4321/mcp
crowley -c crowley.toml                        # transport = "http" in the file
```

Any MCP client that supports the Streamable HTTP transport connects to
`http://<host>:<port>/<http_path>` (default path `/mcp`) with the
`2026-07-28` protocol version. Simple request/response calls are answered
with plain `application/json`; streaming falls back to `text/event-stream`
per spec. Bind `--host 0.0.0.0` to listen on all interfaces (add the
hostname to `allowed_hosts` for DNS-rebinding protection).

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

transport = "stdio"            # stdio | http
host = "127.0.0.1"             # bind address (http transport)
port = 4321                    # bind port; 0 = ephemeral (logged at startup)
http_path = "/mcp"             # URL path for the MCP endpoint
allowed_hosts = [              # Host header allowlist (DNS-rebinding guard)
  "localhost", "127.0.0.1", "::1",
]
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
    --transport <MODE>         stdio (default) | http
    --host <HOST>              bind address for http transport
    --port <PORT>              bind port for http transport (0 = ephemeral)
    --http-path <PATH>         URL path for the http MCP endpoint
    --allowed-host <HOST>      extra allowed Host header (repeatable)
    --print-config             print the effective config as TOML and exit
-v, --verbose                  verbose logging
```

### Inspect the resolved config

```bash
crowley --print-config
crowley -c crowley.toml --output md-fit --print-config
```

### HTTP wire format (for custom clients)

Requests use the `2026-07-28` protocol: send `POST` with
`Content-Type: application/json` and `Accept: application/json, text/event-stream`,
carrying the method in the `Mcp-Method` header and per-request metadata inside
`params._meta`:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/list",
  "params": {
    "_meta": {
      "io.modelcontextprotocol/protocolVersion": "2026-07-28",
      "io.modelcontextprotocol/clientCapabilities": {}
    }
  }
}
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
- `src/main.rs` — CLI, logging, stdio/HTTP serving, shutdown handling

## License

MIT
