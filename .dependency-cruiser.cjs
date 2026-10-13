/**
 * Package layering (GDD 61.7): lower layers never import higher ones.
 *
 *   platform-web < sim-bridge < { sim-host, input, render, audio } < perf < ui < app-shell
 *
 * Packages in the same tier may not import each other. `tools` is build-time only: it may use
 * `platform-web` and `sim-bridge` (to drive the built WASM), and only `app-shell` (the Vite
 * config) may import it.
 */
const TIERS = [
  ['platform-web'],
  ['sim-bridge'],
  ['sim-host', 'input', 'render', 'audio'],
  ['perf'],
  ['ui'],
  ['app-shell'],
];

const pkg = (names) => `^packages/(${names.join('|')})/`;

const upward = TIERS.flatMap((names, i) => {
  const above = TIERS.slice(i + 1).flat();
  return above.length === 0
    ? []
    : [
        {
          name: `tier-${i}-imports-nothing-above`,
          severity: 'error',
          comment: `${names.join(', ')} must not import ${above.join(', ')}`,
          from: { path: pkg(names) },
          to: { path: pkg(above) },
        },
      ];
});

const siblings = TIERS.filter((names) => names.length > 1).flatMap((names) =>
  names.map((name) => ({
    name: `${name}-does-not-import-its-tier-peers`,
    severity: 'error',
    from: { path: pkg([name]) },
    to: { path: pkg(names.filter((n) => n !== name)) },
  })),
);

/** @type {import('dependency-cruiser').IConfiguration} */
module.exports = {
  forbidden: [
    { name: 'no-circular', severity: 'error', from: {}, to: { circular: true } },
    ...upward,
    ...siblings,
    {
      name: 'tools-only-uses-the-bottom-two-layers',
      severity: 'error',
      from: { path: pkg(['tools']) },
      to: {
        path: pkg(['sim-host', 'input', 'render', 'audio', 'perf', 'ui', 'app-shell']),
      },
    },
    {
      name: 'only-app-shell-imports-tools',
      severity: 'error',
      from: {
        path: pkg([
          'platform-web',
          'sim-bridge',
          'sim-host',
          'input',
          'render',
          'audio',
          'perf',
          'ui',
        ]),
      },
      to: { path: pkg(['tools']) },
    },
    {
      name: 'no-test-imports-from-production',
      severity: 'error',
      from: { pathNot: '\\.test\\.ts$' },
      to: { path: '\\.test\\.ts$' },
    },
  ],
  options: {
    includeOnly: '^packages/',
    exclude: { path: '(^|/)node_modules/' },
    tsPreCompilationDeps: true,
    tsConfig: { fileName: 'tsconfig.base.json' },
    enhancedResolveOptions: {
      exportsFields: ['exports'],
      conditionNames: ['import', 'default'],
      extensions: ['.ts', '.js', '.json'],
    },
  },
};
