import * as net from 'net';

interface Envelope<T> {
    type: string;
    payload: T;
}

export class DaemonClient {
    private readonly socketPath: string;

    constructor(socketPath: string) {
        this.socketPath = socketPath;
    }

    send<TReq, TRes>(type: string, payload: TReq): Promise<Envelope<TRes>> {
        return new Promise((resolve, reject) => {
            const socket = net.createConnection(this.socketPath);
            let buffer = '';
            const timeout = setTimeout(() => {
                socket.destroy();
                reject(new Error('senior: daemon request timed out'));
            }, 10_000);

            socket.on('connect', () => {
                const msg = JSON.stringify({ type, payload }) + '\n';
                socket.write(msg);
            });

            socket.on('data', (chunk: Buffer) => {
                buffer += chunk.toString();
                const newlineIdx = buffer.indexOf('\n');
                if (newlineIdx !== -1) {
                    const line = buffer.slice(0, newlineIdx);
                    clearTimeout(timeout);
                    socket.destroy();
                    try {
                        resolve(JSON.parse(line));
                    } catch (e) {
                        reject(new Error(`senior: invalid JSON from daemon: ${line}`));
                    }
                }
            });

            socket.on('error', (err) => {
                clearTimeout(timeout);
                reject(err);
            });
        });
    }

    async ping(): Promise<boolean> {
        try {
            const res = await this.send('ping', null);
            return res.type === 'pong';
        } catch {
            return false;
        }
    }
}
