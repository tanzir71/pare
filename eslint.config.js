// @ts-check
import js from "@eslint/js";
import tseslint from "typescript-eslint";
import reactHooks from "eslint-plugin-react-hooks";
import globals from "globals";

export default tseslint.config(
  {
    ignores: [
      "**/node_modules/**",
      "**/dist/**",
      "**/target/**",
      "**/.astro/**",
      "**/src-tauri/gen/**",
      "packages/tokens/dist/**",
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ["**/*.{js,mjs,cjs}"],
    languageOptions: { globals: { ...globals.node } },
  },
  {
    files: [
      "apps/desktop/src/**/*.{ts,tsx}",
      "packages/ui/src/**/*.{ts,tsx}",
      "apps/web/src/**/*.{ts,tsx}",
    ],
    languageOptions: { globals: { ...globals.browser } },
    plugins: { "react-hooks": reactHooks },
    rules: {
      "react-hooks/rules-of-hooks": "error",
      "react-hooks/exhaustive-deps": "warn",
    },
  },
  {
    // Token-only styling: no raw hex colours in UI components.
    files: [
      "apps/desktop/src/**/*.tsx",
      "apps/web/src/**/*.{ts,tsx}",
      "packages/ui/src/**/*.{ts,tsx}",
    ],
    rules: {
      "no-restricted-syntax": [
        "error",
        {
          // Escape the backslash: in a JS string "\b" is a backspace, so the old selector never matched.
          selector: "Literal[value=/#[0-9a-fA-F]{3,8}\\b/]",
          message:
            "Use token CSS variables (var(--pare-…)) or Tailwind token classes, not raw hex.",
        },
        {
          selector: "TemplateElement[value.raw=/#[0-9a-fA-F]{3,8}\\b/]",
          message:
            "Use token CSS variables (var(--pare-…)) or Tailwind token classes, not raw hex.",
        },
      ],
    },
  },
);
