/** @type {import('dependency-cruiser').IConfiguration} */
module.exports = {
  forbidden: [
    { name: 'no-circular', severity: 'error', from: {}, to: { circular: true } },
    {
      // GDD 61.7: lower layers never import higher ones.
      name: 'libs-do-not-import-app-shell',
      severity: 'error',
      from: { path: '^packages/(platform-web|render|tools)/' },
      to: { path: '^packages/app-shell/' },
    },
    {
      name: 'platform-web-does-not-import-tools',
      severity: 'error',
      from: { path: '^packages/platform-web/' },
      to: { path: '^packages/tools/' },
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
    tsPreCompilationDeps: true,
    tsConfig: { fileName: 'tsconfig.base.json' },
    enhancedResolveOptions: {
      exportsFields: ['exports'],
      conditionNames: ['import', 'default'],
      extensions: ['.ts', '.js', '.json'],
    },
  },
};
