import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import * as vscode from 'vscode';
import * as cp from 'child_process';
import { parseFilesFromDiff, registerCommands } from '../commands';

vi.mock('child_process');

// parseFilesFromDiff is a pure function — no mocking needed.

describe('parseFilesFromDiff', () => {
    it('returns empty array for empty string', () => {
        expect(parseFilesFromDiff('')).toEqual([]);
    });

    it('returns empty array for non-diff text', () => {
        expect(parseFilesFromDiff('some random text\nmore text\n')).toEqual([]);
    });

    it('parses a single-file diff', () => {
        const diff = [
            'diff --git a/src/foo.ts b/src/foo.ts',
            '--- a/src/foo.ts',
            '+++ b/src/foo.ts',
            '@@ -1,3 +1,4 @@',
            ' const x = 1;',
            '-const y = 2;',
            '+const y = 3;',
        ].join('\n');
        expect(parseFilesFromDiff(diff)).toEqual(['src/foo.ts']);
    });

    it('parses multiple files from a multi-file diff', () => {
        const diff = [
            'diff --git a/a.ts b/a.ts',
            '--- a/a.ts',
            '+++ b/a.ts',
            '@@ -1 +1 @@',
            'diff --git a/b.ts b/b.ts',
            '--- a/b.ts',
            '+++ b/b.ts',
        ].join('\n');
        expect(parseFilesFromDiff(diff)).toEqual(['a.ts', 'b.ts']);
    });

    it('parses files in deep nested directories', () => {
        const diff = 'diff --git a/src/utils/helpers/format.ts b/src/utils/helpers/format.ts\n';
        expect(parseFilesFromDiff(diff)).toEqual(['src/utils/helpers/format.ts']);
    });

    it('handles file paths with spaces', () => {
        const diff = 'diff --git a/my file.ts b/my file.ts\n';
        expect(parseFilesFromDiff(diff)).toEqual(['my file.ts']);
    });

    it('handles Rust file extensions', () => {
        const diff = 'diff --git a/daemon/src/main.rs b/daemon/src/main.rs\n';
        expect(parseFilesFromDiff(diff)).toEqual(['daemon/src/main.rs']);
    });

    it('handles unicode characters in file paths', () => {
        const diff = 'diff --git a/src/café.ts b/src/café.ts\n';
        expect(parseFilesFromDiff(diff)).toEqual(['src/café.ts']);
    });

    it('ignores lines that look similar but are not diff --git headers', () => {
        const diff = [
            'diff a/foo.ts b/foo.ts',        // missing --git
            '--- a/foo.ts',
            '+++ b/foo.ts',
            'diff --git a/real.ts b/real.ts', // this one should be picked up
        ].join('\n');
        expect(parseFilesFromDiff(diff)).toEqual(['real.ts']);
    });

    it('returns files in the order they appear in the diff', () => {
        const diff = [
            'diff --git a/z.ts b/z.ts',
            'diff --git a/a.ts b/a.ts',
            'diff --git a/m.ts b/m.ts',
        ].join('\n');
        expect(parseFilesFromDiff(diff)).toEqual(['z.ts', 'a.ts', 'm.ts']);
    });
});

// --- isAnalyzing guard ---
// Tests that concurrent triggers (e.g. rapid auto-saves) don't stack up.

function makeMockManager() {
    return { isRunning: () => true, start: vi.fn(), getSocketPath: () => '/tmp/t.sock' };
}

function makeMockPanel() {
    return { show: vi.fn(), setLoading: vi.fn(), setResult: vi.fn(), setError: vi.fn() };
}

function makeContext() {
    return { subscriptions: { push: vi.fn() } };
}

describe('isAnalyzing guard', () => {
    beforeEach(() => {
        vscode.workspace.workspaceFolders = [
            { uri: { fsPath: '/test-ws' } },
        ] as typeof vscode.workspace.workspaceFolders;
        // Clear command registry between tests
        Object.keys((vscode.commands as any)._registry).forEach(
            k => delete (vscode.commands as any)._registry[k]
        );
    });

    afterEach(() => {
        vi.restoreAllMocks();
    });

    it('second call while first is in-flight does not call panel.show again', async () => {
        // Hold the git-diff callback so call #1 stays suspended at getGitDiff.
        let releaseDiff!: () => void;
        vi.mocked(cp.exec).mockImplementation((cmd: string, _opts: any, cb: any) => {
            if (cmd.includes('rev-parse')) {
                cb(new Error('no parent'), '', '');
            } else {
                // Store callback without calling it — keeps call #1 suspended.
                releaseDiff = () => cb(null, '', ''); // empty diff → setError path
            }
            return {} as any;
        });

        const manager = makeMockManager();
        const panel = makeMockPanel();
        registerCommands(makeContext() as any, manager as any, panel as any);
        const handler = (vscode.commands as any)._registry['senior.explainLastChange'];

        // Call #1: runs synchronously past isAnalyzing=true, panel.show(), then suspends at getGitDiff.
        const first = handler();

        // Call #2: sees isAnalyzing=true immediately — exits without touching the panel.
        const second = handler();

        // panel.show was called exactly once (by call #1, synchronously).
        expect(panel.show).toHaveBeenCalledTimes(1);

        // Release call #1 — empty diff → 'No changes detected' error path, no DaemonClient needed.
        releaseDiff();
        await first;
        await second;

        // Still only one show() call total.
        expect(panel.show).toHaveBeenCalledTimes(1);
        expect(panel.setError).toHaveBeenCalledWith('No changes detected in this repo.');
    });

    it('after first call completes the guard resets and next call proceeds', async () => {
        // Both exec calls return immediately so the handler completes quickly.
        vi.mocked(cp.exec).mockImplementation((cmd: string, _opts: any, cb: any) => {
            if (cmd.includes('rev-parse')) cb(new Error('no parent'), '', '');
            else cb(null, '', ''); // empty diff → setError, no socket call
            return {} as any;
        });

        const manager = makeMockManager();
        const panel = makeMockPanel();
        registerCommands(makeContext() as any, manager as any, panel as any);
        const handler = (vscode.commands as any)._registry['senior.explainLastChange'];

        await handler(); // first call — runs to completion
        expect(panel.show).toHaveBeenCalledTimes(1);

        await handler(); // second call — guard is reset, should proceed normally
        expect(panel.show).toHaveBeenCalledTimes(2);
    });
});
