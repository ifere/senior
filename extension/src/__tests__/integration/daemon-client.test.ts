/**
 * DaemonClient integration tests.
 *
 * Spawns the real daemon binary and sends real requests over the Unix socket.
 * No mocks. These tests catch ECONNREFUSED, stale socket files, protocol
 * mismatches, and any other runtime wiring issues.
 */
import * as cp from 'child_process';
import * as fs from 'fs';
import * as net from 'net';
import * as path from 'path';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { DaemonClient } from '../../daemon/client';

const DAEMON_BIN = path.resolve(
    __dirname,
    '../../../../daemon/target/release/senior-daemon'
);
const TEST_SOCK = '/tmp/senior-ts-integration.sock';

let daemonProcess: cp.ChildProcess;

function waitForSocket(sockPath: string, timeoutMs = 5000): Promise<void> {
    return new Promise((resolve, reject) => {
        const deadline = Date.now() + timeoutMs;
        function attempt() {
            const s = net.createConnection(sockPath);
            s.on('connect', () => { s.destroy(); resolve(); });
            s.on('error', () => {
                if (Date.now() > deadline) {
                    reject(new Error(`daemon socket not ready after ${timeoutMs}ms`));
                } else {
                    setTimeout(attempt, 50);
                }
            });
        }
        attempt();
    });
}

beforeAll(async () => {
    // Remove any stale socket from a previous run
    if (fs.existsSync(TEST_SOCK)) fs.unlinkSync(TEST_SOCK);

    daemonProcess = cp.spawn(DAEMON_BIN, [], {
        env: {
            ...process.env,
            SENIOR_SOCKET_PATH: TEST_SOCK,
            CACTUS_MODEL_PATH: '/nonexistent', // stub mode — no real LLM needed
            RUST_LOG: 'error',
        },
        stdio: 'ignore',
    });

    await waitForSocket(TEST_SOCK);
});

afterAll(() => {
    daemonProcess?.kill();
    if (fs.existsSync(TEST_SOCK)) fs.unlinkSync(TEST_SOCK);
});

function client() {
    return new DaemonClient(TEST_SOCK);
}

// ── Connectivity ──────────────────────────────────────────────────────────────

describe('connectivity', () => {
    it('ping returns pong', async () => {
        const ok = await client().ping();
        expect(ok).toBe(true);
    });

    it('rejects on wrong socket path with ECONNREFUSED', async () => {
        const bad = new DaemonClient('/tmp/does-not-exist.sock');
        await expect(bad.ping()).resolves.toBe(false); // ping() swallows error → false
    });

    it('send() rejects on bad socket path', async () => {
        const bad = new DaemonClient('/tmp/does-not-exist.sock');
        await expect(bad.send('ping', null)).rejects.toThrow(/ENOENT|ECONNREFUSED/);
    });
});

// ── Protocol round-trips ──────────────────────────────────────────────────────

describe('greet request', () => {
    it('returns voice_answer for greet with no analysis', async () => {
        const resp = await client().send<{ last_analysis: null }, { text: string }>(
            'greet',
            { last_analysis: null }
        );
        expect(resp.type).toBe('voice_answer');
        expect(typeof resp.payload.text).toBe('string');
        expect(resp.payload.text.length).toBeGreaterThan(0);
    });

    it('returns voice_answer for greet with analysis context', async () => {
        const analysis = {
            summary: ['touched auth'],
            risk_level: 'high',
            risk_reasons: ['auth path'],
            impacted_files: [{ path: 'src/auth.ts', score: 0.9, why: ['+20 -5'] }],
            impacted_symbols: [],
            suggested_actions: [],
            confidence: 0.85,
        };
        const resp = await client().send<unknown, { text: string }>('greet', {
            last_analysis: analysis,
        });
        expect(resp.type).toBe('voice_answer');
        expect(resp.payload.text.length).toBeGreaterThan(0);
    });
});

describe('voice_query request', () => {
    it('returns voice_answer for a question with no context', async () => {
        const resp = await client().send<unknown, { text: string }>('voice_query', {
            question: 'is this safe to ship?',
            context: null,
        });
        expect(resp.type).toBe('voice_answer');
        expect(resp.payload.text.length).toBeGreaterThan(0);
    });

    it('returns voice_answer for a question with analysis context', async () => {
        const resp = await client().send<unknown, { text: string }>('voice_query', {
            question: 'which file is riskiest?',
            context: {
                summary: ['refactored payments'],
                risk_level: 'high',
                risk_reasons: [],
                impacted_files: [{ path: 'payments.ts', score: 0.95, why: [] }],
                impacted_symbols: [],
                suggested_actions: [],
                confidence: 0.9,
            },
        });
        expect(resp.type).toBe('voice_answer');
        expect(resp.payload.text.length).toBeGreaterThan(0);
    });
});

describe('analyze_diff request', () => {
    const SAMPLE_DIFF = [
        'diff --git a/src/auth.ts b/src/auth.ts',
        'index abc..def 100644',
        '--- a/src/auth.ts',
        '+++ b/src/auth.ts',
        '@@ -1,3 +1,5 @@',
        '+import jwt from "jsonwebtoken";',
        '+export function verify(token: string) { return jwt.verify(token, process.env.SECRET!); }',
        ' export function login() {}',
    ].join('\n');

    it('returns analysis_result in stub mode', async () => {
        const resp = await client().send<unknown, { risk_level: string }>('analyze_diff', {
            diff: SAMPLE_DIFF,
            files_touched: ['src/auth.ts'],
            active_file: 'src/auth.ts',
            trigger: 'manual',
        });
        expect(resp.type).toBe('analysis_result');
        expect(typeof resp.payload.risk_level).toBe('string');
    });

    it('returns error for empty diff gracefully', async () => {
        // The daemon should not crash on empty/minimal input
        const resp = await client().send('analyze_diff', {
            diff: '',
            files_touched: [],
            active_file: '',
            trigger: 'auto',
        });
        // Either an analysis_result (with 0 files) or an error is acceptable; a crash is not
        expect(['analysis_result', 'error']).toContain(resp.type);
    });
});

describe('error handling', () => {
    it('returns error response for unknown request type', async () => {
        const resp = await client().send('not_a_real_type', {});
        expect(resp.type).toBe('error');
        expect((resp.payload as { message: string }).message).toMatch(/parse error/i);
    });

    it('handles concurrent requests without corrupting responses', async () => {
        const results = await Promise.all([
            client().send('ping', null),
            client().send('ping', null),
            client().send('ping', null),
            client().send<{ last_analysis: null }, unknown>('greet', { last_analysis: null }),
        ]);
        expect(results[0].type).toBe('pong');
        expect(results[1].type).toBe('pong');
        expect(results[2].type).toBe('pong');
        expect(results[3].type).toBe('voice_answer');
    });
});
