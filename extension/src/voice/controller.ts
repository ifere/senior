import * as vscode from 'vscode';
import * as cp from 'child_process';
import { DaemonManager } from '../daemon/manager';
import { DaemonClient } from '../daemon/client';

export interface AnalysisResult {
    summary: string[];
    risk_level: string;
    risk_reasons: string[];
    impacted_files: { path: string; score: number; why: string[] }[];
    impacted_symbols: unknown[];
    suggested_actions: { label: string; explanation: string }[];
    confidence: number;
}

export function stripAnsi(text: string): string {
    return text.replace(/\x1B\[[0-9;]*[mGKHFJABCDEFsu]/g, '');
}

export function buildAnalysisSpeech(result: AnalysisResult): string {
    const n = result.impacted_files.length;
    const summary = result.summary.slice(0, 2).join('. ');
    return `Risk is ${result.risk_level}. ${summary}. ${n} file${n !== 1 ? 's' : ''} changed.`;
}

type VoiceState = 'idle' | 'greeting' | 'listening' | 'processing' | 'speaking';

const STATUS_ICONS: Record<VoiceState, string> = {
    idle:       '$(radio-tower) Senior',
    greeting:   '$(unmute) Senior',
    listening:  '$(record) Listening...',
    processing: '$(sync~spin) Thinking...',
    speaking:   '$(unmute) Speaking...',
};

export class VoiceController {
    private state: VoiceState = 'idle';
    private shouldLoop = false;
    private soxProcess: cp.ChildProcess | null = null;
    private sayProcess: cp.ChildProcess | null = null;
    private lastAnalysis: AnalysisResult | null = null;

    constructor(
        private readonly manager: DaemonManager,
        private readonly statusBar: vscode.StatusBarItem,
        private readonly output: vscode.OutputChannel,
    ) {}

    isActive(): boolean {
        return this.state !== 'idle';
    }

    setLastAnalysis(result: AnalysisResult): void {
        this.lastAnalysis = result;
    }

    async toggle(): Promise<void> {
        if (this.state !== 'idle') {
            this.stop();
            return;
        }
        await this.startLoop();
    }

    async speakAnalysis(): Promise<void> {
        if (!this.lastAnalysis) {
            await this.say("You haven't run an analysis yet.");
            return;
        }
        await this.say(buildAnalysisSpeech(this.lastAnalysis));
    }

    stop(): void {
        this.shouldLoop = false;
        this.soxProcess?.kill();
        this.sayProcess?.kill();
        this.soxProcess = null;
        this.sayProcess = null;
        this.setState('idle');
    }

    dispose(): void {
        this.stop();
    }

    private async startLoop(): Promise<void> {
        this.shouldLoop = true;
        this.setState('greeting');

        const greetText = await this.fetchGreeting();
        if (!this.shouldLoop) return;

        await this.say(greetText);

        while (this.shouldLoop) {
            await this.listenOnce();
        }
    }

    private async fetchGreeting(): Promise<string> {
        try {
            const client = new DaemonClient(this.manager.getSocketPath());
            const response = await client.send('greet', { last_analysis: this.lastAnalysis });
            if (response.type === 'voice_answer') {
                return (response.payload as any).text;
            }
        } catch (e) {
            this.output.appendLine(`[voice] greet error: ${e}`);
        }
        return this.lastAnalysis
            ? `Hey, you have ${this.lastAnalysis.impacted_files.length} changed files. What do you want to know?`
            : "Hey, no changes yet. What would you like to work on?";
    }

    private async listenOnce(): Promise<void> {
        this.setState('listening');

        await this.record('/tmp/senior-q.wav');
        if (!this.shouldLoop) return;

        this.setState('processing');

        const question = await this.transcribe('/tmp/senior-q.wav');
        if (!question.trim()) return;

        let answer = "Sorry, I couldn't process that.";
        try {
            const client = new DaemonClient(this.manager.getSocketPath());
            const response = await client.send('voice_query', {
                question,
                context: this.lastAnalysis,
            });
            if (response.type === 'voice_answer') {
                answer = (response.payload as any).text;
            }
        } catch (e) {
            this.output.appendLine(`[voice] query error: ${e}`);
        }

        if (!this.shouldLoop) return;
        this.setState('speaking');
        await this.say(answer);
    }

    private record(outputPath: string): Promise<void> {
        return new Promise((resolve, reject) => {
            this.soxProcess = cp.spawn('sox', [
                '-t', 'coreaudio', 'default',
                '-r', '16000', '-c', '1',
                outputPath,
                'silence', '1', '0.5', '0.1%', '1', '1.5', '0.1%',
            ]);
            this.soxProcess.on('exit', () => resolve());
            this.soxProcess.on('error', (e: NodeJS.ErrnoException) => {
                if (e.code === 'ENOENT') {
                    vscode.window.showErrorMessage(
                        'Senior voice: sox not found. Run: brew install sox'
                    );
                }
                reject(e);
            });
        });
    }

    private transcribe(audioPath: string): Promise<string> {
        return new Promise((resolve) => {
            const asr = this.getAsrBinary();
            const model = this.getSttModelPath();
            const proc = cp.spawn(asr, [model, audioPath]);
            let out = '';
            proc.stdout?.on('data', (d: Buffer) => { out += d.toString(); });
            proc.stderr?.on('data', (d: Buffer) => {
                this.output.appendLine(d.toString().trim());
            });
            proc.on('exit', () => resolve(stripAnsi(out).trim()));
            proc.on('error', (e: NodeJS.ErrnoException) => {
                if (e.code === 'ENOENT') {
                    vscode.window.showErrorMessage(
                        'Senior voice: ASR binary not found. Run: cd /Users/chilly/dev/cactus && venv/bin/cactus build'
                    );
                }
                resolve('');
            });
        });
    }

    private say(text: string): Promise<void> {
        return new Promise((resolve) => {
            this.sayProcess = cp.spawn('say', ['-r', '220', text]);
            this.sayProcess.on('exit', () => resolve());
            this.sayProcess.on('error', () => resolve());
        });
    }

    private setState(s: VoiceState): void {
        this.state = s;
        this.statusBar.text = STATUS_ICONS[s];
    }

    private getAsrBinary(): string {
        return vscode.workspace.getConfiguration('senior').get<string>('asrBinaryPath')
            ?? '/Users/chilly/dev/cactus/tests/build/asr';
    }

    private getSttModelPath(): string {
        return vscode.workspace.getConfiguration('senior').get<string>('sttModelPath')
            ?? '/Users/chilly/dev/cactus/weights/moonshine-base';
    }
}
