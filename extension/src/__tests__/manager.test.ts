import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import * as vscode from 'vscode';
import * as path from 'path';
import { DaemonManager } from '../daemon/manager';

// vscode is aliased to __mocks__/vscode.ts via vitest.config.ts.

function makeContext() {
    return {
        subscriptions: [],
        extensionPath: '/ext',
    } as unknown as vscode.ExtensionContext;
}

function mockConfig(values: Record<string, string>) {
    vi.spyOn(vscode.workspace, 'getConfiguration').mockReturnValue({
        get: <T>(key: string): T | undefined => (values[key] as unknown as T) ?? undefined,
    } as ReturnType<typeof vscode.workspace.getConfiguration>);
}

beforeEach(() => {
    // Reset workspaceFolders to undefined before each test.
    vscode.workspace.workspaceFolders = undefined;
});

afterEach(() => {
    vi.restoreAllMocks();
});

describe('DaemonManager.getDaemonPath', () => {
    it('returns the explicitly configured path when set', () => {
        mockConfig({ daemonPath: '/custom/senior-daemon' });
        const manager = new DaemonManager(makeContext());
        expect(manager.getDaemonPath()).toBe('/custom/senior-daemon');
    });

    it('returns empty string when config is blank and there is no workspace', () => {
        mockConfig({ daemonPath: '' });
        vscode.workspace.workspaceFolders = undefined;
        const manager = new DaemonManager(makeContext());
        expect(manager.getDaemonPath()).toBe('');
    });

    it('returns workspace-relative path when no config is set', () => {
        mockConfig({ daemonPath: '' });
        vscode.workspace.workspaceFolders = [
            { uri: { fsPath: '/my/project' } },
        ] as typeof vscode.workspace.workspaceFolders;
        const manager = new DaemonManager(makeContext());
        const result = manager.getDaemonPath();
        expect(result).toContain('senior-daemon');
        expect(result).toContain('/my/project');
    });

    it('workspace-relative path includes daemon/target/release', () => {
        mockConfig({ daemonPath: '' });
        vscode.workspace.workspaceFolders = [
            { uri: { fsPath: '/projects/myapp' } },
        ] as typeof vscode.workspace.workspaceFolders;
        const manager = new DaemonManager(makeContext());
        const expected = path.join('/projects/myapp', 'daemon', 'target', 'release', 'senior-daemon');
        expect(manager.getDaemonPath()).toBe(expected);
    });

    it('configured path takes priority over workspace path', () => {
        mockConfig({ daemonPath: '/explicit/path/daemon' });
        vscode.workspace.workspaceFolders = [
            { uri: { fsPath: '/some/workspace' } },
        ] as typeof vscode.workspace.workspaceFolders;
        const manager = new DaemonManager(makeContext());
        expect(manager.getDaemonPath()).toBe('/explicit/path/daemon');
    });
});

describe('DaemonManager.getModelPath', () => {
    it('returns the configured model path', () => {
        mockConfig({ modelPath: '/models/functiongemma-270m-it' });
        const manager = new DaemonManager(makeContext());
        expect(manager.getModelPath()).toBe('/models/functiongemma-270m-it');
    });

    it('returns empty string when model path is not configured', () => {
        mockConfig({});
        const manager = new DaemonManager(makeContext());
        expect(manager.getModelPath()).toBe('');
    });
});

describe('DaemonManager.isRunning', () => {
    it('returns false on a fresh instance (daemon not started)', () => {
        mockConfig({});
        const manager = new DaemonManager(makeContext());
        expect(manager.isRunning()).toBe(false);
    });

    it('returns false after stop() is called without start()', () => {
        mockConfig({});
        const manager = new DaemonManager(makeContext());
        manager.stop(); // should not throw
        expect(manager.isRunning()).toBe(false);
    });

    it('dispose() calls stop() and does not throw', () => {
        mockConfig({});
        const manager = new DaemonManager(makeContext());
        expect(() => manager.dispose()).not.toThrow();
        expect(manager.isRunning()).toBe(false);
    });
});

describe('DaemonManager.getSocketPath', () => {
    it('returns the senior socket path', () => {
        mockConfig({});
        const manager = new DaemonManager(makeContext());
        expect(manager.getSocketPath()).toBe('/tmp/senior.sock');
    });
});
