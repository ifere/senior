// Manual mock of the VS Code API.
// Vitest aliases 'vscode' → this file via vitest.config.ts so tests run
// in plain Node without needing an editor host.

export const workspace = {
    getConfiguration: (_section: string) => ({
        get: <T>(_key: string): T | undefined => undefined,
    }),
    workspaceFolders: undefined as { uri: { fsPath: string } }[] | undefined,
    onDidSaveTextDocument: (_handler: unknown) => ({ dispose: () => {} }),
};

export const window = {
    showErrorMessage: (_message: string, ..._args: unknown[]): void => {},
    showInformationMessage: (_message: string, ..._args: unknown[]): void => {},
    createWebviewPanel: (..._args: unknown[]) => ({
        webview: { html: '', onDidReceiveMessage: () => ({ dispose: () => {} }), postMessage: () => {} },
        onDidDispose: () => ({ dispose: () => {} }),
        reveal: () => {},
    }),
    createStatusBarItem: (..._args: unknown[]) => ({
        text: '', tooltip: '', command: '', show() {}, dispose() {},
    }),
    activeTextEditor: undefined as unknown,
};

// Stores handlers registered via registerCommand so tests can invoke them directly.
const _commandRegistry: Record<string, (...args: unknown[]) => unknown> = {};

export const commands = {
    registerCommand: (command: string, cb: (...args: unknown[]) => unknown) => {
        _commandRegistry[command] = cb;
        return { dispose: () => {} };
    },
    executeCommand: (_command: string, ..._args: unknown[]) => Promise.resolve(),
    // Exposed for tests: vscode.commands._registry['senior.explainLastChange']()
    _registry: _commandRegistry,
};

export const StatusBarAlignment = { Left: 1, Right: 2 } as const;
export const ViewColumn = { Active: -1, Beside: -2, One: 1, Two: 2 } as const;

export class Disposable {
    constructor(private _dispose: () => void) {}
    dispose() { this._dispose(); }
    static from(..._items: { dispose: () => unknown }[]) {
        return new Disposable(() => {});
    }
}

export const Uri = {
    file: (fsPath: string) => ({ fsPath }),
};
