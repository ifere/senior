import { describe, it, expect } from 'vitest';
import { parseFilesFromDiff } from '../commands';

// parseFilesFromDiff is a pure function — no mocking needed.

describe('parseFilesFromDiff', () => {
    it('returns empty array for empty string', () => {
        expect(parseFilesFromDiff('')).toEqual([]);
    });

    it('returns empty array for non-diff text', () => {
        expect(parseFilesFromDiff('some random text\nmore text\n')).toEqual([]);
    });

    it('parses a single-file diff', () => {
        const diff = [
            'diff --git a/src/foo.ts b/src/foo.ts',
            '--- a/src/foo.ts',
            '+++ b/src/foo.ts',
            '@@ -1,3 +1,4 @@',
            ' const x = 1;',
            '-const y = 2;',
            '+const y = 3;',
        ].join('\n');
        expect(parseFilesFromDiff(diff)).toEqual(['src/foo.ts']);
    });

    it('parses multiple files from a multi-file diff', () => {
        const diff = [
            'diff --git a/a.ts b/a.ts',
            '--- a/a.ts',
            '+++ b/a.ts',
            '@@ -1 +1 @@',
            'diff --git a/b.ts b/b.ts',
            '--- a/b.ts',
            '+++ b/b.ts',
        ].join('\n');
        expect(parseFilesFromDiff(diff)).toEqual(['a.ts', 'b.ts']);
    });

    it('parses files in deep nested directories', () => {
        const diff = 'diff --git a/src/utils/helpers/format.ts b/src/utils/helpers/format.ts\n';
        expect(parseFilesFromDiff(diff)).toEqual(['src/utils/helpers/format.ts']);
    });

    it('handles file paths with spaces', () => {
        const diff = 'diff --git a/my file.ts b/my file.ts\n';
        expect(parseFilesFromDiff(diff)).toEqual(['my file.ts']);
    });

    it('handles Rust file extensions', () => {
        const diff = 'diff --git a/daemon/src/main.rs b/daemon/src/main.rs\n';
        expect(parseFilesFromDiff(diff)).toEqual(['daemon/src/main.rs']);
    });

    it('handles unicode characters in file paths', () => {
        const diff = 'diff --git a/src/café.ts b/src/café.ts\n';
        expect(parseFilesFromDiff(diff)).toEqual(['src/café.ts']);
    });

    it('ignores lines that look similar but are not diff --git headers', () => {
        const diff = [
            'diff a/foo.ts b/foo.ts',        // missing --git
            '--- a/foo.ts',
            '+++ b/foo.ts',
            'diff --git a/real.ts b/real.ts', // this one should be picked up
        ].join('\n');
        expect(parseFilesFromDiff(diff)).toEqual(['real.ts']);
    });

    it('returns files in the order they appear in the diff', () => {
        const diff = [
            'diff --git a/z.ts b/z.ts',
            'diff --git a/a.ts b/a.ts',
            'diff --git a/m.ts b/m.ts',
        ].join('\n');
        expect(parseFilesFromDiff(diff)).toEqual(['z.ts', 'a.ts', 'm.ts']);
    });
});
