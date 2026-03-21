module.exports = {
  root: true,
  env: { browser: true, es2020: true },
  extends: [
    'eslint:recommended',
    'plugin:@typescript-eslint/recommended',
    'plugin:react-hooks/recommended',
    'prettier',
  ],
  ignorePatterns: ['dist', '.eslintrc.cjs'],
  parser: '@typescript-eslint/parser',
  plugins: ['react-refresh', '@typescript-eslint', 'react-hooks'],
  rules: {
    'react-refresh/only-export-components': [
      'warn',
      { allowConstantExport: true },
    ],
    // Progressive: warn on existing issues, will be tightened later
    '@typescript-eslint/no-unused-vars': [
      'warn',
      { argsIgnorePattern: '^_', varsIgnorePattern: '^_' },
    ],
    '@typescript-eslint/no-explicit-any': 'warn',
    'no-empty': 'warn',
  },
}
