# UX Improvements Design

## Goal

Four targeted improvements to make Senior feel alive from the moment Cursor opens:
1. Auto-greet with voice on startup (contextual — diff-aware)
2. Mute preference: `senior.autoGreet` setting + runtime status-bar toggle (persists via `globalState`)
3. Impact Panel auto-opens on first analysis per session
4. Animated status bar while analysis is running

## Architecture

Option A — incremental fixes. No new abstractions beyond a single `MuteState` class. Each improvement is independently testable and touches a small set of files.

## Components

### 1. `MuteState` (`extension/src/voice/mute-state.ts`) — new file

```typescript
export class MuteState {
    constructor(private ctx: vscode.ExtensionContext) {}

    isEnabled(): boolean {
        const setting = vscode.workspace.getConfiguration('senior').get<boolean>('autoGreet', true);
        const muted = this.ctx.globalState.get<boolean>('senior.muted', false);
        return setting && !muted;
    }

    async toggle(): Promise<void> {
        const muted = this.ctx.globalState.get<boolean>('senior.muted', false);
        await this.ctx.globalState.update('senior.muted', !muted);
    }
}
```

### 2. `VoiceController.autoGreet()` — new method

- Checks `MuteState.isEnabled()` — returns immediately if not
- Speaks a short fixed line (`"Senior ready."`) via `say`
- Runs `git diff HEAD` in workspace root
- If diff non-empty: sends `greet` request to daemon with diff as context → speaks returned text
- If clean: speaks `"Working tree is clean."`
- Entire method wrapped in try/catch — logs to output channel, never throws

### 3. `activate()` in `extension.ts` — one new line

After VoiceController is instantiated:
```typescript
voice.autoGreet(); // fire-and-forget, does not block activation
```

### 4. Status bar animation — `extension.ts` / status bar helper

New `setAnalyzing(busy: boolean)` on the status bar:
- `true`: starts `setInterval(400ms)` cycling `⬤ ○ ○` → `○ ⬤ ○` → `○ ○ ⬤` appended to bar text
- `false`: clears interval, restores normal text

Called around the `client.send('analyze_diff', …)` call in `commands.ts`.

### 5. Panel auto-open — `commands.ts`

After `panel.setResult(result)`:
```typescript
if (!panel.isOpen()) {
    panel.show();
}
```

### 6. `package.json` additions

- `senior.autoGreet`: boolean, default `true`, description "Speak a greeting when the extension activates."
- `senior.toggleMute` command + status-bar click binding

## Data flow

**Startup:**
```
activate()
  └─ voice.autoGreet()
       ├─ MuteState.isEnabled()? no → return
       ├─ say("Senior ready.")
       ├─ git diff HEAD (workspace root)
       ├─ non-empty → greet request → daemon → say(text)
       └─ empty     → say("Working tree is clean.")
```

**On save:**
```
onDidSaveTextDocument
  └─ statusBar.setAnalyzing(true)
  └─ client.send('analyze_diff', …)
  └─ panel.setResult(result)
  └─ if !panel.isOpen() → panel.show()
  └─ voice.setLastAnalysis(result)
  └─ statusBar.setAnalyzing(false)
```

## Error handling

- `autoGreet()` is fully try/caught — any failure (no git, daemon down, no `say`) logs to output channel and returns silently
- Mute toggle is synchronous `globalState` write — no async failure surface
- Status bar animation cleared in `finally` block so it never gets stuck

## Testing

- `MuteState`: unit tests for all combinations of setting + globalState
- `autoGreet()`: unit tests with mocked `execSync`, `say`, and `client.send` — enabled/disabled/muted paths
- Status bar: unit test that `setAnalyzing(true)` mutates text and `setAnalyzing(false)` restores it
- Panel auto-open: extend `commands.test.ts` — `panel.show()` called when `isOpen()` returns false; not called when already open
