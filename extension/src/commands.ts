import * as vscode from 'vscode';
import * as cp from 'child_process';
import { DaemonClient } from './daemon/client';
import { DaemonManager } from './daemon/manager';
import { ImpactPanel } from './ui/panel';

function getGitDiff(workspaceRoot: string): Promise<string> {
    return new Promise((resolve, reject) => {
        cp.exec(
            'git diff HEAD --unified=5',
            { cwd: workspaceRoot, maxBuffer: 1024 * 1024 },
            (err, stdout, stderr) => {
                if (err && !stdout) {
                    reject(new Error(`git diff failed: ${stderr}`));
                    return;
                }
                if (!stdout.trim()) {
                    cp.exec(
                        'git diff HEAD~1 HEAD --unified=5',
                        { cwd: workspaceRoot, maxBuffer: 1024 * 1024 },
                        (_err2, stdout2) => resolve(stdout2 || '')
                    );
                } else {
                    resolve(stdout);
                }
            }
        );
    });
}

export function registerCommands(
    context: vscode.ExtensionContext,
    manager: DaemonManager,
    panel: ImpactPanel
) {
    context.subscriptions.push(
        vscode.commands.registerCommand('callmeout.explainLastChange', async () => {
            const workspaceFolders = vscode.workspace.workspaceFolders;
            if (!workspaceFolders) {
                vscode.window.showErrorMessage('callmeout: No workspace open.');
                return;
            }
            if (!manager.isRunning()) {
                const started = await manager.start();
                if (!started) return;
            }
            const root = workspaceFolders[0].uri.fsPath;
            panel.show();
            panel.setLoading(true);
            try {
                const diff = await getGitDiff(root);
                if (!diff.trim()) {
                    panel.setError('No changes detected.');
                    return;
                }
                const client = new DaemonClient(manager.getSocketPath());
                const response = await client.send('analyze_diff', {
                    diff,
                    files_touched: [],
                    active_file: vscode.window.activeTextEditor?.document.fileName ?? '',
                    trigger: 'manual',
                });
                if (response.type === 'analysis_result') {
                    panel.setResult(response.payload as any);
                } else if (response.type === 'error') {
                    panel.setError((response.payload as any).message);
                }
            } catch (err: any) {
                panel.setError(err.message);
            } finally {
                panel.setLoading(false);
            }
        }),

        vscode.commands.registerCommand('callmeout.showPanel', () => {
            panel.show();
        })
    );
}
