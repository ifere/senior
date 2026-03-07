import * as vscode from 'vscode';

export class MuteState {
    constructor(private readonly ctx: vscode.ExtensionContext) {}

    isEnabled(): boolean {
        const setting = vscode.workspace
            .getConfiguration('senior')
            .get<boolean>('autoGreet', true);
        const muted = this.ctx.globalState.get<boolean>('senior.muted', false);
        return setting === true && muted !== true;
    }

    async toggle(): Promise<void> {
        const muted = this.ctx.globalState.get<boolean>('senior.muted', false);
        await this.ctx.globalState.update('senior.muted', !muted);
    }
}
