import { defineConfig } from 'eslint/config'
import tseslint from '@electron-toolkit/eslint-config-ts'
import eslintConfigPrettier from '@electron-toolkit/eslint-config-prettier'
import eslintPluginReact from 'eslint-plugin-react'
import eslintPluginReactHooks from 'eslint-plugin-react-hooks'
import eslintPluginReactRefresh from 'eslint-plugin-react-refresh'
import eslintPluginImport from 'eslint-plugin-import'

export default defineConfig(
  { ignores: ['**/node_modules', '**/dist', '**/out'] },
  tseslint.configs.recommended,
  eslintPluginReact.configs.flat.recommended,
  eslintPluginReact.configs.flat['jsx-runtime'],
  {
    settings: {
      react: {
        version: 'detect'
      }
    }
  },
  {
    files: ['**/*.{ts,tsx}'],
    plugins: {
      'react-hooks': eslintPluginReactHooks,
      'react-refresh': eslintPluginReactRefresh,
      import: eslintPluginImport
    },
    rules: {
      ...eslintPluginReactHooks.configs.recommended.rules,
      ...eslintPluginReactRefresh.configs.vite.rules,
      // Architecture enforcement rules
      'import/no-restricted-paths': [
        'error',
        {
          zones: [
            // Renderer may only import from shared (types/schemas)
            {
              target: './src/renderer/src/**/*',
              from: './src/main/**/*',
              message: 'Renderer must not import from main process'
            },
            {
              target: './src/renderer/src/**/*',
              from: './electron/**/*',
              message: 'Renderer must not import Electron APIs directly'
            },
            {
              target: './src/renderer/src/**/*',
              from: './node_modules/fs/**/*',
              message: 'Renderer must not import Node.js APIs directly'
            }
          ]
        }
      ],
      // Prevent deep relative imports where path aliases exist
      'import/no-internal-modules': [
        'error',
        {
          allow: [
            // Allow relative imports within the same feature
            'src/renderer/src/features/**',
            'src/main/services/**',
            'src/main/ipc/**'
          ]
        }
      ]
    }
  },
  eslintConfigPrettier
)
