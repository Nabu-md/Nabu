/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    "./index.html",
    "./crates/nabu-ui/src/**/*.rs",
  ],
  theme: {
    extend: {
      colors: {
        primary: "rgb(var(--color-primary) / <alpha-value>)",
        // Nabu's gray scale is backed by CSS variables so dark / light /
        // system themes can swap the entire palette at runtime without
        // touching any component markup.
        gray: {
          50: "rgb(var(--gray-50) / <alpha-value>)",
          100: "rgb(var(--gray-100) / <alpha-value>)",
          200: "rgb(var(--gray-200) / <alpha-value>)",
          300: "rgb(var(--gray-300) / <alpha-value>)",
          400: "rgb(var(--gray-400) / <alpha-value>)",
          500: "rgb(var(--gray-500) / <alpha-value>)",
          600: "rgb(var(--gray-600) / <alpha-value>)",
          700: "rgb(var(--gray-700) / <alpha-value>)",
          750: "rgb(var(--gray-750) / <alpha-value>)",
          800: "rgb(var(--gray-800) / <alpha-value>)",
          900: "rgb(var(--gray-900) / <alpha-value>)",
          950: "rgb(var(--gray-950) / <alpha-value>)",
        },
      },
      fontFamily: {
        sans: [
          "ui-sans-serif",
          "system-ui",
          "-apple-system",
          "BlinkMacSystemFont",
          "\"Segoe UI\"",
          "Roboto",
          "\"Helvetica Neue\"",
          "Arial",
          "sans-serif",
        ],
        mono: [
          "ui-monospace",
          "SFMono-Regular",
          "Menlo",
          "Monaco",
          "Consolas",
          "\"Liberation Mono\"",
          "monospace",
        ],
      },
      borderRadius: {
        card: "0.75rem",
        dialog: "0.875rem",
        panel: "0.5rem",
        chip: "9999px",
      },
      boxShadow: {
        card: "0 1px 2px rgba(0,0,0,0.4), 0 4px 12px rgba(0,0,0,0.35)",
        "card-hover": "0 2px 4px rgba(0,0,0,0.4), 0 8px 24px rgba(0,0,0,0.45)",
        dialog: "0 8px 24px rgba(0,0,0,0.5), 0 24px 64px rgba(0,0,0,0.5)",
        popover: "0 4px 16px rgba(0,0,0,0.45), 0 12px 32px rgba(0,0,0,0.4)",
      },
    },
  },
  plugins: [],
};
