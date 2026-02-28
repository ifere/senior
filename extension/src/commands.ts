import * as vscode from 'vscode';
import * as cp from 'child_process';
import { DaemonClient } from './daemon/client';
import { DaemonManager } from './daemon/manager';
import { ImpactPanel } from './ui/panel';

export function parseFilesFromDiff(diff: string): string[] {
    return diff
        .split('\n')
        .filter(line => line.startsWith('diff --git '))
        .map(line => {
            const m = line.match(/diff --git a\/.+ b\/(.+)/);
            return m ? m[1] : '';
        })
        .filter(Boolean);
}

function hasParentCommit(cwd: string): Promise<boolean> {
    return new Promise(resolve => {
        cp.exec('git rev-parse HEAD~1', { cwd }, err => resolve(!err));
    });
}

function getGitDiff(workspaceRoot: string): Promise<string> {
    return new Promise((resolve, reject) => {
        cp.exec(
            'git diff HEAD --unified=5',
            { cwd: workspaceRoot, maxBuffer: 2 * 1024 * 1024 },
            async (err, stdout, stderr) => {
                if (err && !stdout) {
                    reject(new Error(`git diff failed: ${stderr}`));
                    return;
                }
                if (!stdout.trim()) {
                    // Nothing unstaged — try last commit diff if one exists
                    const hasParent = await hasParentCommit(workspaceRoot);
                    if (!hasParent) {
                        resolve('');
                        return;
                    }
                    cp.exec(
                        'git diff HEAD~1 HEAD --unified=5',
                        { cwd: workspaceRoot, maxBuffer: 2 * 1024 * 1024 },
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
    let isAnalyzing = false;

    context.subscriptions.push(
        vscode.commands.registerCommand('senior.explainLastChange', async () => {
            if (isAnalyzing) return;
            const workspaceFolders = vscode.workspace.workspaceFolders;
            if (!workspaceFolders) {
                vscode.window.showErrorMessage('senior: No workspace open.');
                return;
            }
            if (!manager.isRunning()) {
                const started = await manager.start();
                if (!started) return;
            }
            isAnalyzing = true;
            const root = workspaceFolders[0].uri.fsPath;
            panel.show();
            panel.setLoading(true);
            try {
                const diff = await getGitDiff(root);
                if (!diff.trim()) {
                    panel.setError('No changes detected in this repo.');
                    return;
                }
                const files_touched = parseFilesFromDiff(diff);
                const client = new DaemonClient(manager.getSocketPath());
                const response = await client.send('analyze_diff', {
                    diff,
                    files_touched,
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
                isAnalyzing = false;
                panel.setLoading(false);
            }
        }),

        vscode.commands.registerCommand('senior.showPanel', () => {
            panel.show();
        })
    );
}
