// EI-30 — Single source of truth: palette values come from
// `./src/keep-palette.js`, also imported by `./src/colors.ts`. Update
// hex codes there, not here.
import { LIGHT_HEX, DARK_HEX } from "./src/keep-palette.js";

/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        keep: LIGHT_HEX,
        keepdark: DARK_HEX,
      },
      fontFamily: {
        sans: ["Roboto", "Arial", "system-ui", "sans-serif"],
        product: ["'Product Sans'", "Roboto", "sans-serif"],
      },
      boxShadow: {
        keep: "0 1px 2px 0 rgba(60,64,67,0.302), 0 1px 3px 1px rgba(60,64,67,0.149)",
        "keep-hover": "0 1px 2px 0 rgba(60,64,67,0.302), 0 2px 6px 2px rgba(60,64,67,0.149)",
      },
    },
  },
  plugins: [],
};
