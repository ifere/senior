import { describe, it, expect, vi, afterEach } from 'vitest';
import * as cp from 'child_process';
import * as vscode from 'vscode';
import { VoiceController, buildAnalysisSpeech, stripAnsi } from '../voice/controller';

vi.mock('child_process');

function makeMockManager() {
    return { isRunning: () => true, getSocketPath: () => '/tmp/t.sock' };
}

function makeMockStatusBar() {
    return { text: '', show: vi.fn(), dispose: vi.fn() } as unknown as vscode.StatusBarItem;
}

function makeMockOutput() {
    return { appendLine: vi.fn() } as unknown as vscode.OutputChannel;
}

describe('stripAnsi', () => {
    it('removes ANSI colour codes', () => {
        expect(stripAnsi('\x1B[32mhello\x1B[0m')).toBe('hello');
    });

    it('leaves plain text unchanged', () => {
        expect(stripAnsi('hello world')).toBe('hello world');
    });

    it('removes cursor movement codes', () => {
        expect(stripAnsi('\x1B[2Ksome text\x1B[1A')).toBe('some text');
    });
});

describe('buildAnalysisSpeech', () => {
    it('includes risk level', () => {
        const result = makeResult('med', ['refactored auth'], 1);
        expect(buildAnalysisSpeech(result)).toContain('med');
    });

    it('includes file count singular', () => {
        const result = makeResult('low', ['minor fix'], 1);
        expect(buildAnalysisSpeech(result)).toContain('1 file changed');
    });

    it('includes file count plural', () => {
        const result = makeResult('high', ['big change'], 3);
        expect(buildAnalysisSpeech(result)).toContain('3 files changed');
    });

    it('includes summary text', () => {
        const result = makeResult('low', ['added logging', 'removed debug'], 1);
        const speech = buildAnalysisSpeech(result);
        expect(speech).toContain('added logging');
    });
});

describe('VoiceController', () => {
    afterEach(() => vi.restoreAllMocks());

    it('isActive() returns false on fresh instance', () => {
        const vc = makeVc();
        expect(vc.isActive()).toBe(false);
    });

    it('stop() on idle instance does not throw', () => {
        const vc = makeVc();
        expect(() => vc.stop()).not.toThrow();
        expect(vc.isActive()).toBe(false);
    });

    it('dispose() calls stop() and does not throw', () => {
        const vc = makeVc();
        expect(() => vc.dispose()).not.toThrow();
        expect(vc.isActive()).toBe(false);
    });

    it('setLastAnalysis stores result without throwing', () => {
        const vc = makeVc();
        const result = makeResult('low', ['fix'], 1);
        expect(() => vc.setLastAnalysis(result)).not.toThrow();
    });
});

describe('VoiceController.setAnalyzing()', () => {
    afterEach(() => vi.restoreAllMocks());

    it('setAnalyzing(true) changes statusBar.text to an analyzing frame', () => {
        vi.useFakeTimers();
        const sb = makeMockStatusBar();
        const vc = makeVc(sb);
        vc.setAnalyzing(true);
        expect(sb.text).toMatch(/Senior/);
        vi.useRealTimers();
        vc.setAnalyzing(false);
    });

    it('setAnalyzing(false) restores idle status bar text', () => {
        vi.useFakeTimers();
        const sb = makeMockStatusBar();
        const vc = makeVc(sb);
        vc.setAnalyzing(true);
        vi.advanceTimersByTime(1600);
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

    it('does not throw when voice state is active', () => {
        vi.useFakeTimers();
        const sb = makeMockStatusBar();
        const vc = makeVc(sb);
        expect(() => vc.setAnalyzing(true)).not.toThrow();
        vc.setAnalyzing(false);
        vi.useRealTimers();
    });
});

// --- helpers ---

function makeVc(statusBar?: vscode.StatusBarItem) {
    return new VoiceController(
        makeMockManager() as any,
        statusBar ?? makeMockStatusBar(),
        makeMockOutput(),
    );
}

function makeResult(risk: string, summary: string[], fileCount: number) {
    return {
        summary,
        risk_level: risk,
        risk_reasons: [],
        impacted_files: Array.from({ length: fileCount }, (_, i) => ({
            path: `file${i}.ts`, score: 0.5, why: [],
        })),
        impacted_symbols: [],
        suggested_actions: [],
        confidence: 0.8,
    };
}
