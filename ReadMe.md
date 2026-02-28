# Senior

save a file → know exactly what broke and why, before you even run the tests.

locally. no cloud. instant.

## what it does

every time you hit save, it diffs your repo and asks a local LLM what changed and what it probably affects. results show up in a panel next to your editor — summary, risk level, which files are at risk, what to look at next.

you can also trigger it manually whenever.

## stack

- **TypeScript** — VS Code / Cursor extension (thin client, no logic)
- **Rust + Tokio** — async daemon, does all the heavy lifting
- **Cactus** — on-device LLM inference via C FFI (YC-backed, fast)
- **SQLite** — append-only audit log of every analysis
- **Unix socket** — NDJSON between extension and daemon

```
Cursor/VS Code (TS)  ←→  Daemon (Rust)
                            ├── Cactus LLM (local FFI)
                            ├── Diff parser
                            └── SQLite audit log
```

## setup

### 1. get cactus

you need the cactus SDK built locally and a model downloaded.
see [cactuscompute.com](https://cactuscompute.com) for install instructions.

once you have the dylib, note the path — you'll need it in step 2.

### 2. build the daemon

```bash
cd daemon
CACTUS_LIB_DIR=/path/to/cactus/build cargo build --release
```

set `CACTUS_MODEL_PATH` to your model weights directory (or configure it in VS Code settings and let the extension pass it through).

### 3. install the extension

```bash
cd extension
npm install
npm run package
```

then in Cursor: Extensions → `...` → Install from VSIX → pick `Senior-0.0.1.vsix`

### 4. settings

`Cmd+,` → search Senior:

- `Senior.daemonPath` — path to daemon binary (defaults to `daemon/target/release/Senior-daemon` in workspace root)
- `Senior.modelPath` — model weights dir (passed as `CACTUS_MODEL_PATH` when daemon starts)

### 5. run it

extension starts the daemon automatically on activation. saves trigger analysis with a 1.5s debounce. panel opens beside your editor.

manual: `Cmd+Shift+P` → `Senior: Explain Last Change`

## dev

```bash
cd daemon && cargo test          # run daemon tests
cd extension && npm run watch    # watch TS
make all                         # build both
```

## roadmap

**v0.5** — push-to-talk: Cactus STT → spoken blast-radius summary → `say`

**v1** — apply patch confirmation loop, evidence citations in the panel, full audit log viewer
