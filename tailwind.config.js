/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  darkMode: "class",
  theme: {
    extend: {
      colors: {
        keep: {
          default: "#FFFFFF",
          red: "#FAAFA8",
          orange: "#F39F76",
          yellow: "#FFF8B8",
          green: "#E2F6D3",
          teal: "#B4DDD3",
          blue: "#D4E4ED",
          darkblue: "#AECCDC",
          purple: "#D3BFDB",
          pink: "#F6E2DD",
          brown: "#E9E3D4",
          gray: "#EFEFF1",
        },
        keepdark: {
          default: "#202124",
          red: "#5C2B29",
          orange: "#614A19",
          yellow: "#635D19",
          green: "#345920",
          teal: "#16504B",
          blue: "#2D555E",
          darkblue: "#1E3A5F",
          purple: "#42275E",
          pink: "#5B2245",
          brown: "#442F19",
          gray: "#3C3F43",
        },
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
