import { describe, it, expect, vi, afterEach } from 'vitest';
import * as vscode from 'vscode';
import { MuteState } from '../voice/mute-state';

afterEach(() => vi.restoreAllMocks());

function makeContext(stored: boolean | undefined = undefined) {
    return {
        globalState: {
            get: vi.fn().mockReturnValue(stored),
            update: vi.fn().mockResolvedValue(undefined),
        },
    } as unknown as vscode.ExtensionContext;
}

function stubAutoGreet(value: boolean) {
    vi.spyOn(vscode.workspace, 'getConfiguration').mockReturnValue({
        get: vi.fn().mockReturnValue(value),
    } as any);
}

describe('MuteState.isEnabled()', () => {
    it('returns true when setting is true and not muted', () => {
        stubAutoGreet(true);
        const ms = new MuteState(makeContext(false));
        expect(ms.isEnabled()).toBe(true);
    });

    it('returns false when setting is false even if not muted at runtime', () => {
        stubAutoGreet(false);
        const ms = new MuteState(makeContext(false));
        expect(ms.isEnabled()).toBe(false);
    });

    it('returns false when runtime muted even if setting is true', () => {
        stubAutoGreet(true);
        const ms = new MuteState(makeContext(true));
        expect(ms.isEnabled()).toBe(false);
    });

    it('defaults globalState muted to false when not set', () => {
        stubAutoGreet(true);
        const ms = new MuteState(makeContext(undefined));
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
