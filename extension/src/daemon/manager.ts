import * as vscode from 'vscode';
import * as cp from 'child_process';
import * as path from 'path';
import * as fs from 'fs';
import { DaemonClient } from './client';

export class DaemonManager implements vscode.Disposable {
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

    getModelPath(): string {
        const config = vscode.workspace.getConfiguration('callmeout');
        return config.get<string>('modelPath') ?? '';
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

        const modelPath = this.getModelPath();
        const env: NodeJS.ProcessEnv = {
            ...process.env,
            RUST_LOG: 'info',
            ...(modelPath ? { CACTUS_MODEL_PATH: modelPath } : {}),
        };

        await new Promise<void>((resolve) => {
            this.process = cp.spawn(daemonPath, [], { env });
            this.process.stdout?.on('data', (data: Buffer) => {
                console.log('[callmeout daemon]', data.toString().trim());
            });
            this.process.stderr?.on('data', (data: Buffer) => {
                console.error('[callmeout daemon]', data.toString().trim());
            });
            this.process.on('error', (err) => {
                vscode.window.showErrorMessage(`callmeout daemon error: ${err.message}`);
            });
            // Give the process a moment to bind the socket before we start polling
            setTimeout(resolve, 100);
        });

        // Poll the socket until the daemon is ready (up to 3 seconds)
        const client = new DaemonClient(this.socketPath);
        for (let i = 0; i < 15; i++) {
            if (await client.ping()) return true;
            await new Promise(r => setTimeout(r, 200));
        }

        vscode.window.showErrorMessage('callmeout: daemon started but not responding — check the output panel.');
        return false;
    }

    stop(): void {
        if (this.process && !this.process.killed) {
            this.process.kill();
            this.process = null;
        }
    }

    dispose(): void {
        this.stop();
    }

    isRunning(): boolean {
        return this.process !== null && !this.process.killed;
    }

    getSocketPath(): string {
        return this.socketPath;
    }
}
