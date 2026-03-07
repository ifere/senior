import * as vscode from 'vscode';
import { DaemonManager } from './daemon/manager';
import { ImpactPanel } from './ui/panel';
import { registerCommands } from './commands';
import { VoiceController } from './voice/controller';
import { MuteState } from './voice/mute-state';

let debounceTimer: NodeJS.Timeout | undefined;

export async function activate(context: vscode.ExtensionContext) {
    const output = vscode.window.createOutputChannel('Senior');
    context.subscriptions.push(output);

    const manager = new DaemonManager(context, output);
    const panel = new ImpactPanel(context);

    // manager.dispose() calls manager.stop() automatically when VS Code deactivates
    context.subscriptions.push(manager);

    await manager.start();

    context.subscriptions.push(
        vscode.workspace.onDidSaveTextDocument((_doc) => {
            clearTimeout(debounceTimer);
            debounceTimer = setTimeout(async () => {
                if (!manager.isRunning()) return;
                await vscode.commands.executeCommand('senior.explainLastChange', 'auto');
            }, 1500);
        }),
        // Clear pending debounce when extension deactivates
        { dispose: () => clearTimeout(debounceTimer) }
    );

    const statusBar = vscode.window.createStatusBarItem(
        vscode.StatusBarAlignment.Right,
        100
    );
    statusBar.text = '$(radio-tower) Senior';
    statusBar.tooltip = 'Senior — click to explain last change';
    statusBar.command = 'senior.explainLastChange';
    statusBar.show();
    context.subscriptions.push(statusBar);

    const muteState = new MuteState(context);
    const voice = new VoiceController(manager, statusBar, output, muteState);
    context.subscriptions.push(voice);

    // Fire-and-forget: greet the user on startup if not muted.
    // Never awaited so it doesn't block activation.
    const workspaceRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
    if (workspaceRoot) {
        voice.autoGreet(workspaceRoot);
    }

    registerCommands(context, manager, panel, voice);
}

export function deactivate() {
    // subscriptions are disposed by VS Code automatically;
    // daemon.stop() runs via manager.dispose() in context.subscriptions
}
