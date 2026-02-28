# Senior

> Save a file. Know exactly what you just broke — before you run the tests.

Senior is a Cursor/VS Code extension that analyses your git diff on every save and surfaces a blast-radius report beside your editor: what changed, what it affects, how risky it is, and what to look at next. Everything runs locally via a Rust daemon backed by [Cactus](https://cactuscompute.com) on-device LLM inference. No data leaves your machine.

---

## How it works

```
Cursor / VS Code (TypeScript)
        │  NDJSON over Unix socket
        ▼
  senior-daemon (Rust + Tokio)
        ├── git diff parser
        ├── Cactus LLM  (C FFI → local model)
        └── SQLite audit log
```

When you save a file, the extension diffs the repo, ships the diff to the daemon over a Unix socket, and the daemon runs LLM inference to produce a structured JSON report. The Impact Panel renders the result in a webview beside your editor. The daemon stays alive between saves so inference is always warm.

You can also trigger an analysis at any time via the command palette.

---

## Stack

| Layer | Technology |
|---|---|
| Editor extension | TypeScript, VS Code API |
| Daemon | Rust, Tokio (async runtime) |
| LLM inference | [Cactus SDK](https://cactuscompute.com) via C FFI |
| Audit log | SQLite via rusqlite (bundled) |
| IPC | NDJSON over Unix domain socket |

---

## Prerequisites

- **Rust** ≥ 1.82 with Cargo
- **Node.js** ≥ 18 with npm
- **Cactus SDK** — built locally with a model downloaded. See [cactuscompute.com](https://cactuscompute.com) for installation. Once set up you will have a `libcactus.dylib` (macOS) or `libcactus.so` (Linux) and a model weights directory.

---

## Getting started

### 1. Build the daemon

```bash
cd daemon
CACTUS_LIB_DIR=/path/to/cactus/build cargo build --release
```

`CACTUS_LIB_DIR` points to the directory containing `libcactus.dylib`. If you omit it, the build will warn and fall back to the default dev path — fine for local development, not suitable for distribution.

### 2. Package the extension

```bash
cd extension
npm install
npm run package   # produces senior-0.0.1.vsix
```

### 3. Install in Cursor / VS Code

Open the Extensions panel → `···` menu → **Install from VSIX** → select `senior-0.0.1.vsix`.

### 4. Configure

Open settings (`Cmd+,`) and search **Senior**:

| Setting | Default | Description |
|---|---|---|
| `senior.daemonPath` | *(workspace)* | Path to the `senior-daemon` binary. Defaults to `daemon/target/release/senior-daemon` relative to the workspace root. |
| `senior.modelPath` | — | Path to the Cactus model weights directory. Passed to the daemon as `CACTUS_MODEL_PATH` on startup. |

### 5. Use it

The extension starts the daemon automatically when it activates. Every file save triggers an analysis (1.5 s debounce). The Impact Panel opens beside your editor and shows:

- **Summary** — bullet-point description of what changed
- **Risk level** — `low`, `med`, or `high`, with reasons
- **Impacted files** — click any file to jump to it
- **Suggested actions** — what to check or test next

To trigger manually: `Cmd+Shift+P` → **Senior: Explain Last Change**.

---

## Development

```bash
# Run all daemon unit tests
cd daemon && cargo test

# Watch TypeScript (recompiles on save)
cd extension && npm run watch

# Build both from the repo root
make all
```

The daemon and extension can be developed independently. The daemon exposes a simple NDJSON protocol over `/tmp/senior.sock` — you can send requests manually with `nc -U /tmp/senior.sock` for quick iteration.

### Project structure

```
senior/
├── daemon/               # Rust daemon
│   ├── src/
│   │   ├── main.rs       # Tokio Unix socket server
│   │   ├── protocol.rs   # Request / response types
│   │   ├── analyzer/     # Diff parser + LLM impact analysis
│   │   ├── llm/          # Cactus FFI wrapper
│   │   └── store/        # SQLite audit log
│   └── build.rs          # Links libcactus
└── extension/            # VS Code extension
    └── src/
        ├── extension.ts  # Activation, save hook, status bar
        ├── commands.ts   # Command handlers
        ├── daemon/       # Process manager + socket client
        └── ui/           # Webview panel
```

---

## Contributing

Contributions are welcome. A few things to know before you start:

- **Open an issue first** for anything beyond a small bug fix, so we can agree on the approach before you spend time on it.
- **Tests for the daemon** live in each module under `#[cfg(test)]`. Keep coverage up.
- **The extension is intentionally thin** — business logic belongs in the daemon where it can be tested without a VS Code host.
- Run `cargo test` and `npm run compile` before pushing. Both should be clean.

---

## Roadmap

**v0 — done**
- Save → diff → LLM → Impact Panel
- Local-first via Cactus
- SQLite audit log

**v0.5 — voice**
- Push-to-talk mic capture in the panel
- Cactus Whisper STT → transcribed query
- Spoken blast-radius summary via `say` (macOS) / `espeak` (Linux)

**v1 — action loop**
- Proposed patch confirmation flow (Senior suggests a fix, you approve)
- Evidence citations — which lines triggered each risk flag
- Full audit log panel with replay

---

## License

MIT
