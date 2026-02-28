import * as vscode from 'vscode';
import { DaemonManager } from './daemon/manager';
import { ImpactPanel } from './ui/panel';
import { registerCommands } from './commands';

export async function activate(context: vscode.ExtensionContext) {
    const manager = new DaemonManager(context);
    const panel = new ImpactPanel(context);

    await manager.start();
    registerCommands(context, manager, panel);

    let debounceTimer: NodeJS.Timeout | undefined;
    context.subscriptions.push(
        vscode.workspace.onDidSaveTextDocument((_doc) => {
            clearTimeout(debounceTimer);
            debounceTimer = setTimeout(async () => {
                if (!manager.isRunning()) return;
                await vscode.commands.executeCommand('callmeout.explainLastChange');
            }, 1500);
        })
    );

    const statusBar = vscode.window.createStatusBarItem(
        vscode.StatusBarAlignment.Right,
        100
    );
    statusBar.text = '$(radio-tower) callmeout';
    statusBar.tooltip = 'callmeout Voice Companion — click to explain last change';
    statusBar.command = 'callmeout.explainLastChange';
    statusBar.show();
    context.subscriptions.push(statusBar);
}

export function deactivate() {}
