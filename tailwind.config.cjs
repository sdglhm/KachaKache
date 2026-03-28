/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        ambient: {
          peach: "#f9d8cb",
          cream: "#f7f4ef",
          mist: "#eff2ff",
          lavender: "#c5ccff",
          blue: "#7d92ef",
          indigo: "#5f6ddd",
          slate: "#2a3348",
        },
      },
      fontFamily: {
        sans: [
          "SF Pro Display",
          "SF Pro Text",
          "Inter",
          "ui-sans-serif",
          "system-ui",
          "-apple-system",
          "BlinkMacSystemFont",
          "Segoe UI",
          "sans-serif",
        ],
      },
      boxShadow: {
        float: "0 16px 42px rgba(47, 61, 84, 0.13)",
        glow: "0 0 0 1px rgba(255,255,255,0.65), 0 10px 30px rgba(90, 109, 155, 0.2)",
        orb: "0 18px 48px rgba(88, 102, 230, 0.4), 0 0 0 14px rgba(86, 112, 240, 0.1)",
        coral: "0 18px 45px rgba(243, 88, 88, 0.33), 0 0 0 14px rgba(250, 109, 97, 0.11)",
      },
      borderRadius: {
        glass: "30px",
      },
      keyframes: {
        breathe: {
          "0%, 100%": { transform: "scale(1)", boxShadow: "0 0 0 0 rgba(102, 110, 245, 0.28)" },
          "50%": { transform: "scale(1.02)", boxShadow: "0 0 0 14px rgba(102, 110, 245, 0.1)" },
        },
        pulseSoft: {
          "0%, 100%": { opacity: "0.7" },
          "50%": { opacity: "1" },
        },
      },
      animation: {
        breathe: "breathe 2.8s ease-in-out infinite",
        pulseSoft: "pulseSoft 1.7s ease-in-out infinite",
      },
    },
  },
  plugins: [],
};
