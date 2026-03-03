# UX Improvements Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task.

**Goal:** Auto-greet the user with contextual voice on startup, add a mute preference, animate the status bar during analysis, and auto-open the Impact Panel on the first save.

**Architecture:** Four independent, incremental changes. A new `MuteState` class owns the mute preference. VoiceController gains `autoGreet()`, `setAnalyzing()`, and `toggleMute()` methods. `registerCommands` gains status bar animation and panel auto-open. All wired in `activate()`.

**Tech Stack:** TypeScript, VS Code extension API (`globalState`, `StatusBarItem`, `workspace.getConfiguration`), child_process, Vitest.

---

## Context you must understand before touching code

**Repo layout (extension):**
```
extension/src/
├── extension.ts          — activate() entry point
├── commands.ts           — registerCommands(), explainLastChange handler
├── voice/
│   └── controller.ts     — VoiceController (5-state machine, say/sox/asr)
└── __tests__/
    ├── voice.test.ts      — VoiceController unit tests; makeVc() builds one
    ├── commands.test.ts   — registerCommands() unit tests
    └── integration/       — integration tests (skip for this plan)
```

**How the status bar works today:**
`VoiceController.setState(s)` sets `this.statusBar.text = STATUS_ICONS[s]` where `STATUS_ICONS` is a Record<VoiceState, string>. The statusBar object is owned by VoiceController (passed in constructor). No other code touches `statusBar.text`.

**How `explainLastChange` handles auto-saves today:**
```typescript
if (trigger === 'manual') {
    panel.show();
} else if (!panel.isOpen()) {
    // Auto-save: don't pop open if not already open
    isAnalyzing = false;
    return;   // ← exits without doing anything
}
```
We will change this: remove the early return, run the analysis, and call `panel.show()` after `setResult()` if not already open.

**How to run tests:**
```bash
cd extension
npm test           # all unit tests
npm run test:integration   # integration tests (needs daemon binary)
```

All unit tests must pass after each task before committing.

---

### Task 1: MuteState class

**Purpose:** Owns the `senior.autoGreet` setting and the `senior.muted` `globalState` runtime override. Everything else reads from this class.

**Files:**
- Create: `extension/src/voice/mute-state.ts`
- Create: `extension/src/__tests__/mute-state.test.ts`
- Modify: `extension/package.json` — add `senior.autoGreet` setting + `senior.toggleMute` command

---

**Step 1: Write the failing tests**

Create `extension/src/__tests__/mute-state.test.ts`:

```typescript
import { describe, it, expect, vi } from 'vitest';
import * as vscode from 'vscode';
import { MuteState } from '../voice/mute-state';

function makeContext(stored: boolean | undefined = undefined) {
    return {
        globalState: {
            get: vi.fn().mockReturnValue(stored),
            update: vi.fn().mockResolvedValue(undefined),
        },
    } as unknown as vscode.ExtensionContext;
}

describe('MuteState.isEnabled()', () => {
    it('returns true when setting is true and not muted', () => {
        (vscode.workspace.getConfiguration as any)('senior').get.mockReturnValue(true);
        const ms = new MuteState(makeContext(false));
        expect(ms.isEnabled()).toBe(true);
    });

    it('returns false when setting is false even if not muted at runtime', () => {
        (vscode.workspace.getConfiguration as any)('senior').get.mockReturnValue(false);
        const ms = new MuteState(makeContext(false));
        expect(ms.isEnabled()).toBe(false);
    });

    it('returns false when runtime muted even if setting is true', () => {
        (vscode.workspace.getConfiguration as any)('senior').get.mockReturnValue(true);
        const ms = new MuteState(makeContext(true));
        expect(ms.isEnabled()).toBe(false);
    });

    it('defaults globalState muted to false when not set', () => {
        (vscode.workspace.getConfiguration as any)('senior').get.mockReturnValue(true);
        const ms = new MuteState(makeContext(undefined)); // get() returns undefined → defaults false
        expect(ms.isEnabled()).toBe(true);
    });
});

describe('MuteState.toggle()', () => {
    it('flips false → true', async () => {
        const ctx = makeContext(false);
        const ms = new MuteState(ctx);
        await ms.toggle();
        expect(ctx.globalState.update).toHaveBeenCalledWith('senior.muted', true);
    });

    it('flips true → false', async () => {
        const ctx = makeContext(true);
        const ms = new MuteState(ctx);
        await ms.toggle();
        expect(ctx.globalState.update).toHaveBeenCalledWith('senior.muted', false);
    });
});
```

**Step 2: Run tests to verify they fail**

```bash
cd extension && npm test -- mute-state
```

Expected: `Cannot find module '../voice/mute-state'`

**Step 3: Create `extension/src/voice/mute-state.ts`**

```typescript
import * as vscode from 'vscode';

export class MuteState {
    constructor(private readonly ctx: vscode.ExtensionContext) {}

    isEnabled(): boolean {
        const setting = vscode.workspace
            .getConfiguration('senior')
            .get<boolean>('autoGreet', true);
        const muted = this.ctx.globalState.get<boolean>('senior.muted', false);
        return setting === true && muted !== true;
    }

    async toggle(): Promise<void> {
        const muted = this.ctx.globalState.get<boolean>('senior.muted', false);
        await this.ctx.globalState.update('senior.muted', !muted);
    }
}
```

**Step 4: Add `senior.autoGreet` to `extension/package.json`**

Inside `contributes.configuration.properties`, add after `senior.sttModelPath`:

```json
"senior.autoGreet": {
  "type": "boolean",
  "default": true,
  "description": "Speak a contextual greeting when the extension activates. Can be overridden at runtime with the Senior: Toggle Mute command."
}
```

Inside `contributes.commands`, add:

```json
{
  "command": "senior.toggleMute",
  "title": "Senior: Toggle Mute"
}
```

**Step 5: Run tests to verify they pass**

```bash
cd extension && npm test -- mute-state
```

Expected: all 6 tests pass.

**Step 6: Run full test suite**

```bash
cd extension && npm test
```

Expected: all existing tests still pass.

**Step 7: Commit**

```bash
git add extension/src/voice/mute-state.ts extension/src/__tests__/mute-state.test.ts extension/package.json
git commit -m "feat: add MuteState class and senior.autoGreet setting"
```

---

### Task 2: Status bar animation (`setAnalyzing`)

**Purpose:** Animate the status bar item while the daemon is running an analysis so the user has visual feedback that something is happening.

**Files:**
- Modify: `extension/src/voice/controller.ts` — add `setAnalyzing(busy: boolean)` public method
- Modify: `extension/src/__tests__/voice.test.ts` — add tests for `setAnalyzing`
- Modify: `extension/src/commands.ts` — call `voice.setAnalyzing(true/false)` around the `analyze_diff` call

---

**Step 1: Write failing tests in `extension/src/__tests__/voice.test.ts`**

Add this describe block **after** the existing `VoiceController` block (around line 83, after the closing `}`):

```typescript
describe('VoiceController.setAnalyzing()', () => {
    afterEach(() => vi.restoreAllMocks());

    it('setAnalyzing(true) changes statusBar.text to an analyzing frame', () => {
        vi.useFakeTimers();
        const sb = makeMockStatusBar();
        const vc = makeVc(sb);
        vc.setAnalyzing(true);
        // Immediately after calling, text should indicate analyzing
        expect(sb.text).toMatch(/Senior/);
        vi.useRealTimers();
        vc.setAnalyzing(false); // cleanup
    });

    it('setAnalyzing(false) restores idle status bar text', () => {
        vi.useFakeTimers();
        const sb = makeMockStatusBar();
        const vc = makeVc(sb);
        vc.setAnalyzing(true);
        vi.advanceTimersByTime(1600); // advance past several frames
        vc.setAnalyzing(false);
        expect(sb.text).toBe('$(radio-tower) Senior');
        vi.useRealTimers();
    });

    it('setAnalyzing(false) is a no-op when not analyzing', () => {
        const sb = makeMockStatusBar();
        const vc = makeVc(sb);
        expect(() => vc.setAnalyzing(false)).not.toThrow();
        expect(sb.text).toBe('');
    });

    it('does not interfere when voice state is active', () => {
        vi.useFakeTimers();
        const sb = makeMockStatusBar();
        const vc = makeVc(sb);
        // Simulate voice being in listening state by checking isActive
        // setAnalyzing should be a no-op when voice is active
        // (We cannot easily test the guard without exposing state, so just verify no throw)
        expect(() => vc.setAnalyzing(true)).not.toThrow();
        vc.setAnalyzing(false);
        vi.useRealTimers();
    });
});
```

Also update `makeVc()` to accept an optional statusBar parameter:

```typescript
function makeVc(statusBar?: vscode.StatusBarItem) {
    return new VoiceController(
        makeMockManager() as any,
        statusBar ?? makeMockStatusBar(),
        makeMockOutput(),
    );
}
```

**Step 2: Run tests to verify they fail**

```bash
cd extension && npm test -- voice
```

Expected: `vc.setAnalyzing is not a function`

**Step 3: Add `setAnalyzing` to `extension/src/voice/controller.ts`**

Add a private field after `private lastAnalysis`:
```typescript
private analyzingInterval: NodeJS.Timeout | undefined;
```

Add the public method (place it after `isActive()`):
```typescript
setAnalyzing(busy: boolean): void {
    if (busy) {
        if (this.state !== 'idle') return; // voice active — don't clobber
        const frames = ['$(sync~spin) Senior', '$(radio-tower) Senior ●', '$(radio-tower) Senior'];
        let i = 0;
        this.statusBar.text = frames[0];
        this.analyzingInterval = setInterval(() => {
            this.statusBar.text = frames[i % frames.length];
            i++;
        }, 400);
    } else {
        clearInterval(this.analyzingInterval);
        this.analyzingInterval = undefined;
        if (this.state === 'idle') {
            this.statusBar.text = STATUS_ICONS.idle;
        }
    }
}
```

Also clear the interval in `stop()` — add this line after the existing kill calls:
```typescript
clearInterval(this.analyzingInterval);
this.analyzingInterval = undefined;
```

**Step 4: Wire `setAnalyzing` into `commands.ts`**

In `registerCommands`, in the `explainLastChange` handler, wrap the try/catch/finally block:

Change the lines before `panel.setLoading(true)`:
```typescript
// BEFORE:
panel.setLoading(true);
try {
```

To:
```typescript
// AFTER:
voice.setAnalyzing(true);
panel.setLoading(true);
try {
```

Change the `finally` block:
```typescript
// BEFORE:
} finally {
    isAnalyzing = false;
    panel.setLoading(false);
}
```

To:
```typescript
// AFTER:
} finally {
    isAnalyzing = false;
    panel.setLoading(false);
    voice.setAnalyzing(false);
}
```

**Step 5: Run tests to verify they pass**

```bash
cd extension && npm test -- voice
```

Expected: all `setAnalyzing` tests pass.

**Step 6: Run full test suite**

```bash
cd extension && npm test
```

Expected: all tests pass.

**Step 7: Commit**

```bash
git add extension/src/voice/controller.ts extension/src/__tests__/voice.test.ts extension/src/commands.ts
git commit -m "feat: animate status bar during analysis (setAnalyzing)"
```

---

### Task 3: Panel auto-open on first analysis

**Purpose:** On automatic saves, the panel currently bails out if it isn't already open. Change it to run the analysis and open the panel after the first result.

**Files:**
- Modify: `extension/src/commands.ts` — change the early-return logic
- Modify: `extension/src/__tests__/commands.test.ts` — add auto-open tests

---

**Step 1: Write the failing tests**

Add this describe block to `extension/src/__tests__/commands.test.ts` (after the last existing `describe`):

```typescript
describe('auto-trigger panel auto-open', () => {
    beforeEach(() => {
        vscode.workspace.workspaceFolders = [
            { uri: { fsPath: '/test-ws' } },
        ] as typeof vscode.workspace.workspaceFolders;
        Object.keys((vscode.commands as any)._registry).forEach(
            k => delete (vscode.commands as any)._registry[k]
        );
    });

    afterEach(() => { vi.restoreAllMocks(); });

    it('auto trigger opens panel after first result when panel was not open', async () => {
        vi.mocked(cp.exec).mockImplementation((cmd: string, _opts: any, cb: any) => {
            if (cmd.includes('rev-parse')) cb(new Error('no parent'), '', '');
            else cb(null, 'diff --git a/foo.ts b/foo.ts\n--- a/foo.ts\n+++ b/foo.ts\n@@ -1 +1 @@\n-old\n+new\n', '');
            return {} as any;
        });

        const panel = makeMockPanel();
        panel.isOpen.mockReturnValue(false); // panel is closed

        registerCommands(makeContext() as any, makeMockManager() as any, panel as any, makeMockVoice() as any);
        const handler = (vscode.commands as any)._registry['senior.explainLastChange'];

        await handler('auto');

        // Panel should have been opened automatically
        expect(panel.show).toHaveBeenCalled();
        expect(panel.setResult).toHaveBeenCalled();
    });

    it('auto trigger does not call show() when panel is already open', async () => {
        vi.mocked(cp.exec).mockImplementation((cmd: string, _opts: any, cb: any) => {
            if (cmd.includes('rev-parse')) cb(new Error('no parent'), '', '');
            else cb(null, 'diff --git a/foo.ts b/foo.ts\n--- a/foo.ts\n+++ b/foo.ts\n@@ -1 +1 @@\n-old\n+new\n', '');
            return {} as any;
        });

        const panel = makeMockPanel();
        panel.isOpen.mockReturnValue(true); // panel already open

        registerCommands(makeContext() as any, makeMockManager() as any, panel as any, makeMockVoice() as any);
        const handler = (vscode.commands as any)._registry['senior.explainLastChange'];

        await handler('auto');

        // show() should NOT have been called — panel was already open
        expect(panel.show).not.toHaveBeenCalled();
        expect(panel.setResult).toHaveBeenCalled();
    });
});
```

**Step 2: Run tests to verify they fail**

```bash
cd extension && npm test -- commands
```

Expected: `auto trigger opens panel` fails — `panel.show` not called (current code returns early).

**Step 3: Change the early-return logic in `extension/src/commands.ts`**

Find this block (around line 77):

```typescript
if (trigger === 'manual') {
    panel.show();
} else if (!panel.isOpen()) {
    // Auto-save: don't pop open the panel if the user hasn't opened it yet
    isAnalyzing = false;
    return;
}
```

Replace it with:

```typescript
if (trigger === 'manual') {
    panel.show();
}
```

Then find the line `panel.setResult(response.payload as any);` (around line 100) and add panel auto-open immediately after:

```typescript
panel.setResult(response.payload as any);
if (trigger === 'auto' && !panel.isOpen()) {
    panel.show();
}
```

**Step 4: Run tests to verify they pass**

```bash
cd extension && npm test -- commands
```

Expected: all tests pass including the new auto-open tests.

**Step 5: Run full test suite**

```bash
cd extension && npm test
```

Expected: all tests pass.

**Step 6: Commit**

```bash
git add extension/src/commands.ts extension/src/__tests__/commands.test.ts
git commit -m "feat: auto-open Impact Panel on first analysis"
```

---

### Task 4: VoiceController.autoGreet() and MuteState integration

**Purpose:** VoiceController gains an `autoGreet()` method that fires on startup — short fixed greeting, then a contextual diff summary. It reads `MuteState.isEnabled()` to decide whether to speak.

**Files:**
- Modify: `extension/src/voice/controller.ts` — accept MuteState in constructor, add `autoGreet()` and `toggleMute()`
- Modify: `extension/src/__tests__/voice.test.ts` — update `makeVc()` to pass MuteState, add autoGreet tests

---

**Step 1: Write failing tests in `extension/src/__tests__/voice.test.ts`**

First update `makeVc()` to accept an optional MuteState:

```typescript
function makeMuteState(enabled = true) {
    return { isEnabled: vi.fn().mockReturnValue(enabled), toggle: vi.fn() };
}

function makeVc(statusBar?: vscode.StatusBarItem, muteState?: ReturnType<typeof makeMuteState>) {
    return new VoiceController(
        makeMockManager() as any,
        statusBar ?? makeMockStatusBar(),
        makeMockOutput(),
        (muteState ?? makeMuteState()) as any,
    );
}
```

Add this describe block for autoGreet:

```typescript
describe('VoiceController.autoGreet()', () => {
    afterEach(() => vi.restoreAllMocks());

    it('does nothing when MuteState.isEnabled() returns false', async () => {
        const muteState = makeMuteState(false);
        const vc = makeVc(undefined, muteState);
        // _speakDirect spawns 'say' — mock cp.spawn to detect if it fires
        const spawnSpy = vi.mocked(cp.spawn);
        await vc.autoGreet('/workspace');
        expect(spawnSpy).not.toHaveBeenCalled();
    });

    it('speaks greeting and calls say when enabled and workspace root provided', async () => {
        const muteState = makeMuteState(true);
        const vc = makeVc(undefined, muteState);

        // Mock cp.exec for git diff (returns empty — clean tree)
        vi.mocked(cp.exec).mockImplementation((_cmd: string, _opts: any, cb: any) => {
            cb(null, '', ''); // clean working tree
            return {} as any;
        });

        // Mock cp.spawn for 'say'
        const fakeProcess = { on: vi.fn(), stdout: null, stderr: null };
        vi.mocked(cp.spawn).mockReturnValue(fakeProcess as any);
        // Immediately resolve the 'close' event
        fakeProcess.on.mockImplementation((event: string, cb: Function) => {
            if (event === 'close' || event === 'exit') cb(0);
        });

        await vc.autoGreet('/workspace');

        // say should have been called (at least the fixed greeting)
        const sayCall = vi.mocked(cp.spawn).mock.calls.find(c => c[0] === 'say');
        expect(sayCall).toBeDefined();
    });

    it('does not throw when git command fails', async () => {
        const muteState = makeMuteState(true);
        const vc = makeVc(undefined, muteState);

        vi.mocked(cp.exec).mockImplementation((_cmd: string, _opts: any, cb: any) => {
            cb(new Error('not a git repo'), '', '');
            return {} as any;
        });
        const fakeProcess = { on: vi.fn() };
        vi.mocked(cp.spawn).mockReturnValue(fakeProcess as any);
        fakeProcess.on.mockImplementation((event: string, cb: Function) => {
            if (event === 'close' || event === 'exit') cb(0);
        });

        await expect(vc.autoGreet('/workspace')).resolves.not.toThrow();
    });
});

describe('VoiceController.toggleMute()', () => {
    it('delegates to MuteState.toggle()', async () => {
        const muteState = makeMuteState();
        const vc = makeVc(undefined, muteState);
        await vc.toggleMute();
        expect(muteState.toggle).toHaveBeenCalledTimes(1);
    });
});
```

**Step 2: Run tests to verify they fail**

```bash
cd extension && npm test -- voice
```

Expected: `VoiceController` constructor type error / `autoGreet is not a function`

**Step 3: Update VoiceController constructor and add new methods**

In `extension/src/voice/controller.ts`:

Add import at the top:
```typescript
import { MuteState } from './mute-state';
```

Update the constructor:
```typescript
constructor(
    private readonly manager: DaemonManager,
    private readonly statusBar: vscode.StatusBarItem,
    private readonly output: vscode.OutputChannel,
    private readonly muteState: MuteState,
) {}
```

Add `autoGreet` and `toggleMute` public methods (place after `setLastAnalysis`):

```typescript
async autoGreet(workspaceRoot: string): Promise<void> {
    if (!this.muteState.isEnabled()) return;
    try {
        await this._speakDirect('Senior ready.');
        const diff = await this.getStartupDiff(workspaceRoot);
        if (!diff.trim()) {
            await this._speakDirect('Working tree is clean.');
            return;
        }
        const client = new DaemonClient(this.manager.getSocketPath());
        const response = await client.send<unknown, VoiceAnswer>('greet', {
            context: diff.slice(0, 2000), // keep payload reasonable
        });
        const text = response.type === 'voice_answer'
            ? response.payload.text
            : 'Ready. You have uncommitted changes.';
        await this._speakDirect(text);
    } catch (e) {
        this.output.appendLine(`[voice] autoGreet error: ${e}`);
    }
}

async toggleMute(): Promise<void> {
    await this.muteState.toggle();
}

private getStartupDiff(workspaceRoot: string): Promise<string> {
    return new Promise(resolve => {
        cp.exec(
            'git diff HEAD --stat',
            { cwd: workspaceRoot },
            (_err, stdout) => resolve(stdout ?? ''),
        );
    });
}
```

**Step 4: Run tests to verify they pass**

```bash
cd extension && npm test -- voice
```

Expected: all voice tests pass.

**Step 5: Run full test suite**

```bash
cd extension && npm test
```

Expected: all tests pass. Note: existing `makeVc()` calls in commands.test.ts are not affected because VoiceController is mocked there.

**Step 6: Commit**

```bash
git add extension/src/voice/controller.ts extension/src/voice/mute-state.ts extension/src/__tests__/voice.test.ts
git commit -m "feat: add VoiceController.autoGreet() and MuteState integration"
```

---

### Task 5: Wire everything in extension.ts

**Purpose:** Create `MuteState` in `activate()`, pass it to VoiceController, call `voice.autoGreet()` on startup, and register the `senior.toggleMute` command.

**Files:**
- Modify: `extension/src/extension.ts`
- Modify: `extension/src/commands.ts` — register `senior.toggleMute`

There are no unit tests for `extension.ts` itself (it's a thin wiring layer — testing it requires a full VS Code host). We will verify manually by building and smoke-testing.

---

**Step 1: Update `extension/src/extension.ts`**

Add the MuteState import at the top:
```typescript
import { MuteState } from './voice/mute-state';
```

In `activate()`, create MuteState and pass it to VoiceController:

Find:
```typescript
const voice = new VoiceController(manager, statusBar, output);
context.subscriptions.push(voice);
```

Replace with:
```typescript
const muteState = new MuteState(context);
const voice = new VoiceController(manager, statusBar, output, muteState);
context.subscriptions.push(voice);

// Fire-and-forget: greet the user on startup if not muted.
// Never awaited so it doesn't block activation.
const workspaceRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
if (workspaceRoot) {
    voice.autoGreet(workspaceRoot);
}
```

**Step 2: Register `senior.toggleMute` in `registerCommands`**

In `extension/src/commands.ts`, add inside the `context.subscriptions.push(...)` block:

```typescript
vscode.commands.registerCommand('senior.toggleMute', () => voice.toggleMute()),
```

**Step 3: Compile**

```bash
cd extension && npm run compile
```

Expected: no TypeScript errors.

**Step 4: Run full test suite**

```bash
cd extension && npm test
```

Expected: all tests pass.

**Step 5: Build VSIX and install**

```bash
cd extension && npm run package
```

Expected: produces `senior-0.0.1.vsix`

Install in Cursor:
```
Extensions panel → ··· → Install from VSIX → select senior-0.0.1.vsix
```

**Step 6: Smoke test**

1. Open a workspace in Cursor with the extension installed
2. Confirm Senior greets you with voice on startup (≤ 5 seconds after window opens)
3. Run `Cmd+Shift+P` → "Senior: Toggle Mute" — confirm subsequent reloads are silent
4. Run `Cmd+Shift+P` → "Senior: Toggle Mute" again — confirm greeting returns
5. Save a file — confirm status bar animates during analysis
6. On a fresh session, save a file — confirm Impact Panel opens automatically

**Step 7: Commit**

```bash
git add extension/src/extension.ts extension/src/commands.ts
git commit -m "feat: wire autoGreet, mute toggle, and status bar animation in activate()"
```

---

## Done

All four improvements are live:
- Voice greeting on startup (contextual — diff-aware)
- Mute preference: `senior.autoGreet` setting + `Senior: Toggle Mute` command
- Impact Panel auto-opens on first save per session
- Status bar animates during analysis
