import { execFile } from 'node:child_process';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';

const execFileAsync = promisify(execFile);

export async function loadLiveDashboard() {
  const repositoryRoot = await gitRoot();
  const tempRoot = await mkdtemp(path.join(os.tmpdir(), 'codexu-playwright-live-'));
  const outputPath = path.join(tempRoot, 'dashboard.json');
  const cachePath = path.join(tempRoot, 'cache');

  try {
    await execFileAsync(
      process.env.CARGO_BIN ?? 'cargo.exe',
      [
        'run',
        '--quiet',
        '--manifest-path',
        path.join(repositoryRoot, 'windows', 'Cargo.toml'),
        '--package',
        'codexu-cli',
        '--',
        '--dashboard',
        '--output',
        outputPath,
        '--cache-dir',
        cachePath,
      ],
      {
        cwd: path.join(repositoryRoot, 'windows'),
        env: { ...process.env, RUST_LOG: 'warn' },
        maxBuffer: 16 * 1024 * 1024,
      },
    );

    const dashboard = JSON.parse(await readFile(outputPath, 'utf8'));
    return {
      dashboard,
      settings: {
        config: {
          codex_root: '<local Codex data>',
          cache_dir: '<local codexU cache>',
          theme: 'light',
          refresh_interval_secs: 60,
          tray_density: 'classic',
          language: 'en',
          palette_id: 'codexu.default',
        },
        app_data_dir: '<local codexU app data>',
      },
      source: {
        mode: 'live-readonly',
        provider: 'codex-dashboard',
      },
    };
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
}

export async function getRepositoryIdentity() {
  const repositoryRoot = await gitRoot();
  const [{ stdout: branch }, { stdout: head }] = await Promise.all([
    execFileAsync(process.env.GIT_BIN ?? 'git.exe', ['branch', '--show-current'], {
      cwd: repositoryRoot,
      encoding: 'utf8',
    }),
    execFileAsync(process.env.GIT_BIN ?? 'git.exe', ['rev-parse', 'HEAD'], {
      cwd: repositoryRoot,
      encoding: 'utf8',
    }),
  ]);

  return { branch: branch.trim(), head: head.trim() };
}

async function gitRoot() {
  const { stdout } = await execFileAsync(process.env.GIT_BIN ?? 'git.exe', ['rev-parse', '--show-toplevel'], {
    cwd: path.resolve(fileURLToPath(new URL('.', import.meta.url)), '../../../../../..'),
    encoding: 'utf8',
  });
  return stdout.trim();
}
