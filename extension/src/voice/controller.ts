import * as vscode from 'vscode';
import * as cp from 'child_process';
import { DaemonManager } from '../daemon/manager';
import { DaemonClient } from '../daemon/client';
import { MuteState } from './mute-state';

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

// Minor 6: named constant for the temporary WAV path
const VOICE_TMP_WAV = '/tmp/senior-q.wav';

// Minor 5: typed interface for voice_answer payload
interface VoiceAnswer { text: string }

export class VoiceController {
    private state: VoiceState = 'idle';
    private shouldLoop = false;
    private soxProcess: cp.ChildProcess | null = null;
    private sayProcess: cp.ChildProcess | null = null;
    private asrProcess: cp.ChildProcess | null = null;  // Fix 2: track ASR process
    private lastAnalysis: AnalysisResult | null = null;
    private analyzingInterval: NodeJS.Timeout | undefined;

    constructor(
        private readonly manager: DaemonManager,
        private readonly statusBar: vscode.StatusBarItem,
        private readonly output: vscode.OutputChannel,
        private readonly muteState: MuteState,
    ) {}

    isActive(): boolean {
        return this.state !== 'idle';
    }

    setAnalyzing(busy: boolean): void {
        if (busy) {
            if (this.state !== 'idle') return; // voice active — don't clobber
            const frames = ['$(sync~spin) Senior', '$(radio-tower) Senior ●', '$(radio-tower) Senior'];
            let i = 0;
            this.statusBar.text = frames[0];
            this.analyzingInterval = setInterval(() => {
                this.statusBar.text = frames[i % frames.length];
                i++;
            }, 400);
        } else if (this.analyzingInterval !== undefined) {
            clearInterval(this.analyzingInterval);
            this.analyzingInterval = undefined;
            if (this.state === 'idle') {
                this.statusBar.text = STATUS_ICONS.idle;
            }
        }
    }

    setLastAnalysis(result: AnalysisResult): void {
        this.lastAnalysis = result;
    }

    async autoGreet(workspaceRoot: string): Promise<void> {
        if (!this.muteState.isEnabled()) return;
        try {
            await this._speakDirect('Senior ready.');
            const diff = await this.getStartupDiff(workspaceRoot);
            if (!diff.trim()) {
                await this._speakDirect('Working tree is clean.');
                return;
            }
            const client = new DaemonClient(this.manager.getSocketPath());
            const response = await client.send<unknown, VoiceAnswer>('greet', {
                context: diff.slice(0, 2000),
            });
            const text = response.type === 'voice_answer'
                ? response.payload.text
                : 'Ready. You have uncommitted changes.';
            await this._speakDirect(text);
        } catch (e) {
            this.output.appendLine(`[voice] autoGreet error: ${e}`);
        }
    }

    async toggleMute(): Promise<void> {
        await this.muteState.toggle();
    }

    async toggle(): Promise<void> {
        if (this.state !== 'idle') {
            this.stop();
            return;
        }
        await this.startLoop();
    }

    // Fix 3: speakAnalysis() uses _speakDirect() to avoid clobbering this.sayProcess
    async speakAnalysis(): Promise<void> {
        if (!this.lastAnalysis) {
            await this._speakDirect('No analysis available yet.');
            return;
        }
        await this._speakDirect(buildAnalysisSpeech(this.lastAnalysis));
    }

    stop(): void {
        this.shouldLoop = false;
        this.soxProcess?.kill();
        this.sayProcess?.kill();
        this.asrProcess?.kill();  // Fix 2: kill ASR process on stop
        this.soxProcess = null;
        this.sayProcess = null;
        this.asrProcess = null;
        clearInterval(this.analyzingInterval);
        this.analyzingInterval = undefined;
        this.setState('idle');
    }

    dispose(): void {
        this.stop();
    }

    private getStartupDiff(workspaceRoot: string): Promise<string> {
        return new Promise(resolve => {
            cp.exec(
                'git diff HEAD --stat',
                { cwd: workspaceRoot },
                (_err, stdout) => resolve(stdout ?? ''),
            );
        });
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
            const response = await client.send<unknown, VoiceAnswer>('greet', { last_analysis: this.lastAnalysis });
            if (response.type === 'voice_answer') {
                return response.payload.text;
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

        // Fix 1: wrap record() in try/catch so errors exit the loop cleanly
        try {
            await this.record(VOICE_TMP_WAV);
        } catch {
            this.stop();
            return;
        }
        if (!this.shouldLoop) return;

        this.setState('processing');

        const question = await this.transcribe(VOICE_TMP_WAV);
        if (!question.trim()) return;

        let answer = "Sorry, I couldn't process that.";
        try {
            const client = new DaemonClient(this.manager.getSocketPath());
            const response = await client.send<unknown, VoiceAnswer>('voice_query', {
                question,
                context: this.lastAnalysis,
            });
            if (response.type === 'voice_answer') {
                answer = response.payload.text;
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
            // Fix 2: store process on this so stop() can kill it
            this.asrProcess = cp.spawn(asr, [model, audioPath]);
            const proc = this.asrProcess;
            let out = '';
            proc.stdout?.on('data', (d: Buffer) => { out += d.toString(); });
            proc.stderr?.on('data', (d: Buffer) => {
                this.output.appendLine(d.toString().trim());
            });
            proc.on('exit', () => resolve(stripAnsi(out).trim()));
            proc.on('error', (e: NodeJS.ErrnoException) => {
                if (e.code === 'ENOENT') {
                    // Minor 7: user-friendly ASR error message without developer path
                    vscode.window.showErrorMessage(
                        'Senior voice: ASR binary not found. Run: cactus build (see https://github.com/cactus-compute/cactus)'
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

    // Fix 3: _speakDirect() uses a local process, never touches this.sayProcess
    private _speakDirect(text: string): Promise<void> {
        return new Promise((resolve) => {
            const proc = cp.spawn('say', ['-r', '220', text]);
            proc.on('close', resolve);
            proc.on('error', resolve);
        });
    }

    private setState(s: VoiceState): void {
        this.state = s;
        this.statusBar.text = STATUS_ICONS[s];
    }

    private getAsrBinary(): string {
        return vscode.workspace.getConfiguration('senior').get<string>('asrBinaryPath') ?? '';
    }

    private getSttModelPath(): string {
        return vscode.workspace.getConfiguration('senior').get<string>('sttModelPath') ?? '';
    }
}
