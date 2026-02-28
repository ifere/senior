import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import * as net from 'net';
import { DaemonClient } from '../daemon/client';

vi.mock('net');

// Helper: create a mock socket that fires events after all handlers are registered.
function makeMockSocket(opts: {
    response?: string;       // raw string sent back (including newline)
    connectError?: Error;    // emit error instead of connect
}) {
    const handlers: Record<string, (arg?: unknown) => void> = {};
    const socket = {
        on: vi.fn().mockImplementation((event: string, cb: (arg?: unknown) => void) => {
            handlers[event] = cb;
            return socket;
        }),
        write: vi.fn(),
        destroy: vi.fn(),
    };

    // Fire events after handler registration is complete (next microtask).
    setImmediate(() => {
        if (opts.connectError) {
            handlers['error']?.(opts.connectError);
        } else {
            handlers['connect']?.();
            if (opts.response !== undefined) {
                setImmediate(() => handlers['data']?.(Buffer.from(opts.response!)));
            }
        }
    });

    return socket;
}

beforeEach(() => {
    vi.resetAllMocks();
});

afterEach(() => {
    vi.restoreAllMocks();
});

describe('DaemonClient.ping', () => {
    it('returns true when daemon responds with pong', async () => {
        vi.mocked(net.createConnection).mockReturnValue(
            makeMockSocket({ response: '{"type":"pong"}\n' }) as any
        );
        const client = new DaemonClient('/tmp/senior.sock');
        expect(await client.ping()).toBe(true);
    });

    it('returns false when connection is refused', async () => {
        vi.mocked(net.createConnection).mockReturnValue(
            makeMockSocket({ connectError: new Error('ECONNREFUSED') }) as any
        );
        const client = new DaemonClient('/tmp/senior.sock');
        expect(await client.ping()).toBe(false);
    });

    it('returns false when daemon responds with non-pong type', async () => {
        vi.mocked(net.createConnection).mockReturnValue(
            makeMockSocket({ response: '{"type":"error","payload":{"message":"oops"}}\n' }) as any
        );
        const client = new DaemonClient('/tmp/senior.sock');
        expect(await client.ping()).toBe(false);
    });
});

describe('DaemonClient.send', () => {
    it('resolves with parsed JSON on a valid response', async () => {
        const payload = { summary: ['changed auth'], risk_level: 'low' };
        const responseJson = JSON.stringify({ type: 'analysis_result', payload }) + '\n';
        vi.mocked(net.createConnection).mockReturnValue(
            makeMockSocket({ response: responseJson }) as any
        );
        const client = new DaemonClient('/tmp/senior.sock');
        const result = await client.send('analyze_diff', { diff: 'test' });
        expect(result.type).toBe('analysis_result');
        expect((result.payload as any).risk_level).toBe('low');
    });

    it('rejects when the daemon sends invalid JSON', async () => {
        vi.mocked(net.createConnection).mockReturnValue(
            makeMockSocket({ response: 'not valid json\n' }) as any
        );
        const client = new DaemonClient('/tmp/senior.sock');
        await expect(client.send('ping', null)).rejects.toThrow('invalid JSON');
    });

    it('rejects when the socket emits an error', async () => {
        vi.mocked(net.createConnection).mockReturnValue(
            makeMockSocket({ connectError: new Error('ENOENT: socket not found') }) as any
        );
        const client = new DaemonClient('/tmp/senior.sock');
        await expect(client.send('ping', null)).rejects.toThrow('ENOENT');
    });

    it('writes the correct NDJSON envelope to the socket', async () => {
        const mockSocket = makeMockSocket({ response: '{"type":"pong"}\n' });
        vi.mocked(net.createConnection).mockReturnValue(mockSocket as any);
        const client = new DaemonClient('/tmp/senior.sock');
        await client.send('ping', null);
        expect(mockSocket.write).toHaveBeenCalledOnce();
        const written = mockSocket.write.mock.calls[0][0] as string;
        const parsed = JSON.parse(written.trim());
        expect(parsed.type).toBe('ping');
        expect(parsed.payload).toBeNull();
    });

    it('destroys the socket after receiving a response', async () => {
        const mockSocket = makeMockSocket({ response: '{"type":"pong"}\n' });
        vi.mocked(net.createConnection).mockReturnValue(mockSocket as any);
        const client = new DaemonClient('/tmp/senior.sock');
        await client.send('ping', null);
        expect(mockSocket.destroy).toHaveBeenCalled();
    });
});
