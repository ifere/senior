# callmeout

AI pair programmer for Cursor. Saves to file → instant blast-radius analysis → Impact Panel shows what changed, what's at risk, and what to do next.

## What it does

- **Explain Last Change** — on every save (or manually), diffs your repo and asks the local LLM: "what changed and what does it affect?"
- **Impact Panel** — shows summary bullets, risk level (low/med/high), impacted files (click to open), and suggested next actions
- **Local-first** — all inference runs on-device via [Cactus](https://cactuscompute.com/). No data leaves your machine.

## Architecture

```
Extension (TypeScript)  ←→  Daemon (Rust)
                               ├── Cactus LLM (local FFI)
                               ├── Diff parser (tree-sitter ready)
                               └── SQLite audit log
```

Communication: NDJSON over Unix socket (`/tmp/callmeout.sock`).

## Setup

### 1. Install Cactus

```bash
# Clone and set up Cactus
git clone https://github.com/cactus-compute/cactus /Users/chilly/dev/cactus
cd /Users/chilly/dev/cactus && source ./setup

# Download a model
cactus download LiquidAI/LFM2.5-1.2B-Instruct
# or use the bundled model: weights/functiongemma-270m-it
```

### 2. Build the daemon

```bash
cd daemon
cargo build --release
```

The daemon links against `libcactus.dylib`. Override the library path with:
```bash
CACTUS_LIB_DIR=/path/to/cactus/build cargo build --release
```

### 3. Install the extension

```bash
cd extension
npm install
npm run package   # produces callmeout-0.0.1.vsix
```

Install in Cursor: Extensions panel → `...` → Install from VSIX → select `callmeout-0.0.1.vsix`

### 4. Configure

In VS Code/Cursor settings (`Cmd+,` → search "callmeout"):

| Setting | Default | Description |
|---------|---------|-------------|
| `callmeout.daemonPath` | auto | Path to daemon binary (defaults to `daemon/target/release/callmeout-daemon` in workspace) |
| `callmeout.modelPath` | — | Path to model weights dir (used for `CACTUS_MODEL_PATH` env var when starting daemon) |

### 5. Run

The extension automatically:
1. Starts the daemon on activation
2. Analyzes diffs on every file save (1.5s debounce)
3. Shows the Impact Panel beside your editor

Or trigger manually: `Cmd+Shift+P` → `callmeout: Explain Last Change`

## Development

```bash
# Run all daemon tests
cd daemon && cargo test

# Watch extension TypeScript
cd extension && npm run watch

# Build both
make all
```

## Roadmap

- **v0.5** — Push-to-talk voice: Cactus STT → spoken summary via macOS `say`
- **v1** — `apply_patch` confirmation flow, evidence citations, full audit log panel
