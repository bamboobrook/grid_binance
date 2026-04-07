import type { Config } from "tailwindcss";

const config: Config = {
  darkMode: ["class", '[data-theme="dark"]'],
  content: [
    "./app/**/*.{js,ts,jsx,tsx,mdx}",
    "./components/**/*.{js,ts,jsx,tsx,mdx}",
  ],
  theme: {
    extend: {
      colors: {
        background: "#0f172a", // Very dark slate (3commas background)
        foreground: "#f8fafc",
        card: {
          DEFAULT: "#1e293b", // Slate 800
          foreground: "#f8fafc",
        },
        popover: {
          DEFAULT: "#1e293b",
          foreground: "#f8fafc",
        },
        primary: {
          DEFAULT: "#3b82f6", // Blue 500 (3commas primary brand)
          foreground: "#ffffff",
        },
        secondary: {
          DEFAULT: "#334155", // Slate 700
          foreground: "#f8fafc",
        },
        muted: {
          DEFAULT: "#1e293b",
          foreground: "#94a3b8", // Slate 400
        },
        accent: {
          DEFAULT: "#3b82f6",
          foreground: "#ffffff",
        },
        destructive: {
          DEFAULT: "#ef4444", // Red 500
          foreground: "#f8fafc",
        },
        border: "#334155", // Slate 700
        input: "#1e293b",
        ring: "#3b82f6",
        positive: "#10b981", // Emerald 500 (3commas green)
        negative: "#ef4444", // Red 500 (3commas red)
        warning: "#f59e0b", // Amber 500
      },
      borderRadius: {
        lg: "0.5rem",
        md: "0.375rem",
        sm: "0.25rem",
      },
    },
  },
  plugins: [],
};

export default config;
