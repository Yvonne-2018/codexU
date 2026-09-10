import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import ts from 'typescript';

const here = path.dirname(fileURLToPath(import.meta.url));
const webRoot = path.resolve(here, '..');
const read = (relativePath) => fs.readFileSync(path.join(webRoot, relativePath), 'utf8');
const readJson = (relativePath) => JSON.parse(read(relativePath));

const surfaceTokens = [
  '--window-bg',
  '--page-bg',
  '--toolbar-bg',
  '--card-bg',
  '--card-bg-strong',
  '--control-bg',
  '--focus-surface',
  '--status-surface',
  '--glass-fallback-bg',
];

const escapeRegExp = (value) => value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

async function loadTsModule(relativePath) {
  const source = read(relativePath);
  const output = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.ESNext,
      resolveJsonModule: true,
      target: ts.ScriptTarget.ES2022,
    },
  }).outputText;
  return import(`data:text/javascript,${encodeURIComponent(output)}`);
}

test('main Windows Tauri window opts into transparent glass composition', () => {
  const tauriConfig = JSON.parse(read('../src-tauri/tauri.conf.json'));
  const mainWindow = tauriConfig.app.windows.find((windowConfig) => windowConfig.label === 'main');

  assert.equal(mainWindow.transparent, true);
  assert.equal(mainWindow.decorations, true);
});

test('Windows light and dark themes publish every layered glass surface token', () => {
  const css = read('src/index.css');
  const theme = read('src/utils/appTheme.ts');
  const mapping = readJson('src/utils/windowsGlassSurfaceMap.json');

  for (const token of surfaceTokens) {
    const tokenPattern = escapeRegExp(token);
    assert.match(css, new RegExp(`:root[\\s\\S]*${tokenPattern}:`, 'u'));
    assert.match(css, new RegExp(`\\.dark[\\s\\S]*${tokenPattern}:`, 'u'));
  }

  assert.deepEqual(
    mapping.layers.map((layer) => layer.cssVariable),
    surfaceTokens.filter((token) => token !== '--glass-fallback-bg'),
  );
  assert.match(theme, /windowsGlassSurfaceMap\.layers/u);
  assert.match(theme, /resolveWindowsGlassVisualTokens\(isDark,\s*palette\.surfaceTint\)/u);
  assert.match(theme, /palette\.surfaceTint\.color/u);
  assert.match(theme, /palette\.surfaceTint\.maximumOpacity/u);
});

test('window, page, toolbar, card, control, focus, and status layers use distinct glass tokens with readable fallback', () => {
  const css = read('src/index.css');
  const dashboard = read('src/windows/Dashboard.tsx');
  const app = read('src/App.tsx');

  assert.match(css, /body\s*\{[\s\S]*background:\s*var\(--window-bg\)/u);
  assert.match(dashboard, /windows-glass-page/u);
  assert.match(app, /windows-glass-page/u);

  assert.match(css, /\.glass-toolbar\s*\{[\s\S]*background:\s*var\(--toolbar-bg\)/u);
  assert.match(css, /\.glass-panel\s*\{[\s\S]*background:\s*var\(--card-bg\)/u);
  assert.match(css, /\.glass-panel-strong\s*\{[\s\S]*background:\s*var\(--card-bg-strong\)/u);
  assert.match(css, /\.glass-button\s*\{[\s\S]*background:\s*var\(--control-bg\)/u);
  assert.match(css, /:focus-visible\s*\{[\s\S]*outline:\s*2px solid var\(--focus-surface\)/u);
  assert.match(css, /\.dashboard-overview-status\s*\{[\s\S]*background:\s*var\(--status-surface\)/u);
  assert.match(css, /@supports not \(\(backdrop-filter:\s*blur\(1px\)\) or \(-webkit-backdrop-filter:\s*blur\(1px\)\)\)/u);
  assert.match(css, /--toolbar-bg:\s*var\(--glass-fallback-bg\)/u);
});

test('machine-readable glass surface map separates CSS fallback from native composition evidence', () => {
  const mapping = readJson('src/utils/windowsGlassSurfaceMap.json');

  assert.deepEqual(
    mapping.layers.map((layer) => layer.id),
    ['window', 'page', 'toolbar', 'card', 'cardStrong', 'control', 'focus', 'status'],
  );
  assert.deepEqual(
    mapping.layers.map((layer) => layer.cssVariable),
    [
      '--window-bg',
      '--page-bg',
      '--toolbar-bg',
      '--card-bg',
      '--card-bg-strong',
      '--control-bg',
      '--focus-surface',
      '--status-surface',
    ],
  );
  assert.equal(mapping.fallbacks.cssBackdropFilter.status, 'source_contract');
  assert.equal(mapping.fallbacks.nativeTransparency.status, 'not_observed_by_css_fallback');
  assert.equal(mapping.fallbacks.dwmComposition.status, 'not_observed_by_css_fallback');
  assert.equal(mapping.fallbacks.webview2Transparency.status, 'not_observed_by_css_fallback');
});

test('CSS fallback gives window, page, and status layers opaque readable surfaces', () => {
  const css = read('src/index.css');
  const mapping = readJson('src/utils/windowsGlassSurfaceMap.json');
  const fallbackBlock = css.slice(css.indexOf('@supports not ((backdrop-filter: blur(1px))'));
  const fallbackLayers = mapping.fallbacks.cssBackdropFilter.layers;

  assert.deepEqual(fallbackLayers, [
    { id: 'window', selector: 'body', cssVariable: '--glass-fallback-window-bg', opacity: 'opaque' },
    { id: 'page', selector: '.windows-glass-page', cssVariable: '--glass-fallback-page-bg', opacity: 'opaque' },
    { id: 'status', selector: '.dashboard-overview-status', cssVariable: '--glass-fallback-status-bg', opacity: 'opaque' },
  ]);

  for (const layer of fallbackLayers) {
    assert.match(
      fallbackBlock,
      new RegExp(`${escapeRegExp(layer.selector)}[\\s\\S]*background:\\s*var\\(${escapeRegExp(layer.cssVariable)}\\)`, 'u'),
    );
  }

  for (const variable of ['--glass-fallback-window-bg', '--glass-fallback-page-bg', '--glass-fallback-status-bg']) {
    assert.match(css, new RegExp(`${escapeRegExp(variable)}:\\s*#[0-9a-f]{6};`, 'iu'));
  }
});

test('surfaceTint maximum opacity participates in resolved page rendering token', async () => {
  const { resolveWindowsGlassVisualTokens } = await loadTsModule('src/utils/windowsGlassTokens.ts');
  const lowOpacity = resolveWindowsGlassVisualTokens(false, {
    color: '#336699',
    maximumOpacity: 0.05,
  });
  const highOpacity = resolveWindowsGlassVisualTokens(false, {
    color: '#336699',
    maximumOpacity: 0.3,
  });

  assert.equal(lowOpacity.paletteSurfaceTintActive, '#3366990D');
  assert.equal(highOpacity.paletteSurfaceTintActive, '#3366994D');
  assert.notEqual(lowOpacity.pageBg, highOpacity.pageBg);
  assert.match(highOpacity.pageBg, /#3366994D/u);
});

test('Settings roots expose a visual-only glass page surface marker', () => {
  const settingsSource = read('src/windows/Settings.tsx');
  const sourceFile = ts.createSourceFile(
    'Settings.tsx',
    settingsSource,
    ts.ScriptTarget.Latest,
    true,
    ts.ScriptKind.TSX,
  );
  const pageRoots = [];

  const visit = (node) => {
    if (ts.isJsxOpeningElement(node)) {
      const className = node.attributes.properties.find(
        (property) => ts.isJsxAttribute(property) && property.name.text === 'className',
      );
      const dataSurface = node.attributes.properties.find(
        (property) => ts.isJsxAttribute(property) && property.name.text === 'data-glass-surface',
      );
      const classLiteral =
        className && ts.isStringLiteral(className.initializer) ? className.initializer.text : '';
      if (classLiteral.includes('windows-glass-page')) {
        pageRoots.push(dataSurface && ts.isStringLiteral(dataSurface.initializer) ? dataSurface.initializer.text : null);
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(sourceFile);

  assert.deepEqual(pageRoots, ['page', 'page', 'page']);
  assert.equal(settingsSource.includes('open_settings_window'), false);
});
