import js from "@eslint/js";
import tseslint from "typescript-eslint";
import eslintConfigPrettier from "eslint-config-prettier";
import eslintPluginReact from "eslint-plugin-react";
import eslintPluginReactHooks from "eslint-plugin-react-hooks";
import eslintPluginReactRefresh from "eslint-plugin-react-refresh";
import eslintPluginImportX from "eslint-plugin-import-x";

export default [
  { ignores: ["**/node_modules", "**/dist", "**/out", "src-backup", "extensions"] },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  eslintPluginReact.configs.flat.recommended,
  eslintPluginReact.configs.flat["jsx-runtime"],
  {
    settings: {
      react: {
        version: "detect",
      },
    },
  },
  {
    files: ["**/*.{ts,tsx}"],
    plugins: {
      "react-hooks": eslintPluginReactHooks,
      "react-refresh": eslintPluginReactRefresh,
      import: eslintPluginImportX,
    },
    rules: {
      ...eslintPluginReactHooks.configs.recommended.rules,
      ...eslintPluginReactRefresh.configs.vite.rules,
      // TODO: Restore the following rules to "error" after Platform Modernization Wave 4.
      "react-hooks/exhaustive-deps": "warn",
      "@typescript-eslint/no-unused-vars": "warn",
      "@typescript-eslint/no-explicit-any": "warn",
      "react-hooks/rules-of-hooks": "error",
      "react-hooks/set-state-in-effect": "warn",
      // Architecture enforcement rules
      "import/no-restricted-paths": [
        "error",
        {
          zones: [
            {
              target: "./src/renderer/src/**/*",
              from: "./src/main/**/*",
              message: "Renderer must not import from main process",
            },
            {
              target: "./src/renderer/src/**/*",
              from: "./electron/**/*",
              message: "Renderer must not import Electron APIs directly",
            },
            {
              target: "./src/renderer/src/**/*",
              from: "./node_modules/fs/**/*",
              message: "Renderer must not import Node.js APIs directly",
            },
          ],
        },
      ],
      // Prevent deep relative imports where path aliases exist
      "import/no-internal-modules": [
        "warn",
        {
          allow: [
            "src/renderer/src/features/**",
            "src/main/services/**",
            "src/main/ipc/**",
          ],
        },
      ],
    },
  },
  eslintConfigPrettier,
  {
    files: ["scripts/*.js"],
    languageOptions: {
      globals: {
        process: "readonly",
        console: "readonly",
        __dirname: "readonly"
      }
    }
  }
];
