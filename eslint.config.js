import js from "@eslint/js";
import globals from "globals";
import tseslint from "typescript-eslint";
import reactHooks from "eslint-plugin-react-hooks";

/** @type {import('eslint').Linter.Config[]} */
export default [
  {
    ignores: [
      "dist/**",
      "dist-portable/**",
      "dist-web-clipper/**",
      "src-tauri/target/**",
      "node_modules/**",
      "**/*.d.ts",
      "vite.config.js",
      "vite.config.d.ts",
      "tsconfig.tsbuildinfo",
      "tsconfig.node.tsbuildinfo",
      "*.tsbuildinfo",
      // v0.24.0 — the browser-extension code under `web-clipper/` runs
      // inside Chrome/Edge/Firefox, not Node and not our Vite renderer.
      // It uses its own globals (chrome, browser, fetch, window) that
      // this config doesn't surface. The extension code is small + flat
      // enough that linting it under a browser env wouldn't catch
      // anything our manual review didn't.
      "web-clipper/**",
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ["scripts/**/*.mjs"],
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: "module",
      globals: { ...globals.node, ...globals.es2022 },
    },
  },
  {
    files: ["src/**/*.{ts,tsx}"],
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: "module",
      globals: { ...globals.browser, ...globals.es2022 },
      parserOptions: { ecmaFeatures: { jsx: true } },
    },
    plugins: {
      "react-hooks": reactHooks,
    },
    rules: {
      // The rule that would have caught v0.16.1's editor-blanks-screen bug.
      "react-hooks/rules-of-hooks": "error",
      "react-hooks/exhaustive-deps": "warn",

      // We use TypeScript for everything; the unused-vars TS variant is
      // configured below to also exempt _-prefixed args.
      "no-unused-vars": "off",
      "@typescript-eslint/no-unused-vars": [
        "warn",
        { argsIgnorePattern: "^_", varsIgnorePattern: "^_" },
      ],
      // The codebase has a few intentional `any` escapes at the Tauri IPC
      // boundary (payload serialization) — keep this a warning, not error.
      "@typescript-eslint/no-explicit-any": "warn",
    },
  },
];
