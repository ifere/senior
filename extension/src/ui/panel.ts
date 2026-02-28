import * as vscode from 'vscode';
import * as fs from 'fs';
import * as path from 'path';
import * as crypto from 'crypto';

export class ImpactPanel {
    private panel: vscode.WebviewPanel | null = null;
    private readonly context: vscode.ExtensionContext;

    constructor(context: vscode.ExtensionContext) {
        this.context = context;
    }

    show(): void {
        if (this.panel) {
            this.panel.reveal(vscode.ViewColumn.Beside);
            return;
        }
        this.panel = vscode.window.createWebviewPanel(
            'callmeoutPanel',
            'callmeout',
            vscode.ViewColumn.Beside,
            { enableScripts: true, retainContextWhenHidden: true }
        );
        this.panel.webview.html = this.getHtml();
        this.panel.webview.onDidReceiveMessage((msg) => {
            if (msg.type === 'openFile') {
                const folders = vscode.workspace.workspaceFolders;
                if (!folders) return;
                const filePath = path.isAbsolute(msg.path)
                    ? msg.path
                    : path.join(folders[0].uri.fsPath, msg.path);
                const uri = vscode.Uri.file(filePath);
                vscode.window.showTextDocument(uri, { preview: false });
            }
        });
        this.panel.onDidDispose(() => { this.panel = null; });
    }

    setLoading(loading: boolean): void {
        this.panel?.webview.postMessage({ type: loading ? 'loading' : 'idle' });
    }

    setResult(result: unknown): void {
        this.panel?.webview.postMessage({ type: 'result', result });
    }

    setError(message: string): void {
        this.panel?.webview.postMessage({ type: 'error', message });
    }

    private getHtml(): string {
        const htmlPath = path.join(this.context.extensionPath, 'src', 'ui', 'panel.html');
        const nonce = crypto.randomBytes(16).toString('hex');
        return fs.readFileSync(htmlPath, 'utf8').replace(/NONCE/g, nonce);
    }
}
