import * as vscode from 'vscode';
import * as cp from 'child_process';
import * as path from 'path';
import * as fs from 'fs';

export class DaemonManager {
    private process: cp.ChildProcess | null = null;
    private readonly socketPath = '/tmp/callmeout.sock';

    constructor(private readonly context: vscode.ExtensionContext) {}

    getDaemonPath(): string {
        const config = vscode.workspace.getConfiguration('callmeout');
        const configured = config.get<string>('daemonPath');
        if (configured && configured.length > 0) {
            return configured;
        }
        const workspaceFolders = vscode.workspace.workspaceFolders;
        if (workspaceFolders && workspaceFolders.length > 0) {
            return path.join(
                workspaceFolders[0].uri.fsPath,
                'daemon',
                'target',
                'release',
                'callmeout-daemon'
            );
        }
        return '';
    }

    async start(): Promise<boolean> {
        const daemonPath = this.getDaemonPath();
        if (!daemonPath || !fs.existsSync(daemonPath)) {
            vscode.window.showErrorMessage(
                `callmeout: daemon binary not found at "${daemonPath}". ` +
                `Run "cargo build --release" in the daemon/ directory.`
            );
            return false;
        }
        if (this.process && !this.process.killed) {
            return true;
        }
        return new Promise((resolve) => {
            this.process = cp.spawn(daemonPath, [], {
                env: { ...process.env, RUST_LOG: 'info' },
            });
            this.process.stdout?.on('data', (data: Buffer) => {
                console.log('[callmeout daemon]', data.toString().trim());
            });
            this.process.stderr?.on('data', (data: Buffer) => {
                console.error('[callmeout daemon]', data.toString().trim());
            });
            this.process.on('error', (err) => {
                vscode.window.showErrorMessage(`callmeout daemon error: ${err.message}`);
                resolve(false);
            });
            setTimeout(() => resolve(true), 500);
        });
    }

    stop(): void {
        if (this.process && !this.process.killed) {
            this.process.kill();
            this.process = null;
        }
    }

    isRunning(): boolean {
        return this.process !== null && !this.process.killed;
    }

    getSocketPath(): string {
        return this.socketPath;
    }
}
