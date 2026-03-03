/**
 * Prerequisites integration tests.
 *
 * Checks that the system tools the extension relies on are actually installed
 * and reachable. These tests have no mocks — a failure here means something
 * real is missing from the environment and will break the extension at runtime.
 */
import { execSync, spawnSync } from 'child_process';
import * as fs from 'fs';
import * as path from 'path';
import { describe, it, expect } from 'vitest';

const DAEMON_BIN = path.resolve(
    __dirname,
    '../../../../daemon/target/release/senior-daemon'
);

describe('daemon binary', () => {
    it('release binary exists and is executable', () => {
        expect(fs.existsSync(DAEMON_BIN), `binary not found at ${DAEMON_BIN}`).toBe(true);
        const stat = fs.statSync(DAEMON_BIN);
        // Check execute bit (owner)
        expect(stat.mode & 0o100, 'binary is not executable').toBeGreaterThan(0);
    });
});

describe('sox', () => {
    it('sox is installed and reports a version', () => {
        const result = spawnSync('sox', ['--version'], { encoding: 'utf8' });
        expect(result.status, 'sox exited with non-zero status').toBe(0);
        const output = (result.stdout ?? '') + (result.stderr ?? '');
        expect(output, 'sox --version produced no output').toMatch(/SoX/i);
    });

    it('sox coreaudio input type is available on this platform', () => {
        // Check that sox was built with coreaudio support (macOS only).
        // If this fails, voice recording will silently produce empty files.
        const result = spawnSync('sox', ['--help'], { encoding: 'utf8' });
        const output = (result.stdout ?? '') + (result.stderr ?? '');
        expect(output, 'sox does not list coreaudio as an available type').toMatch(/coreaudio/i);
    });
});

describe('say (macOS TTS)', () => {
    it('say binary exists at /usr/bin/say', () => {
        expect(fs.existsSync('/usr/bin/say'), 'say TTS not found at /usr/bin/say').toBe(true);
    });

    it('say accepts a dry-run flag without error', () => {
        // -v '?' lists voices — a quick no-audio smoke test
        const result = spawnSync('say', ['-v', '?'], { encoding: 'utf8' });
        expect(result.status, 'say -v ? exited with non-zero status').toBe(0);
    });
});

describe('cactus ASR binary', () => {
    const ASR_PATHS = [
        process.env.SENIOR_ASR_BINARY,
        '/Users/chilly/dev/cactus/tests/build/asr',
    ].filter(Boolean) as string[];

    it('at least one configured ASR binary path exists', () => {
        const found = ASR_PATHS.find(p => fs.existsSync(p));
        expect(
            found,
            `No ASR binary found. Checked: ${ASR_PATHS.join(', ')}. Run: cactus build`
        ).toBeTruthy();
    });

    it('ASR model weights directory exists', () => {
        const MODEL_PATHS = [
            process.env.SENIOR_STT_MODEL,
            '/Users/chilly/dev/cactus/weights/moonshine-base',
        ].filter(Boolean) as string[];

        const found = MODEL_PATHS.find(p => fs.existsSync(p));
        expect(
            found,
            `No STT model found. Checked: ${MODEL_PATHS.join(', ')}`
        ).toBeTruthy();
    });
});
