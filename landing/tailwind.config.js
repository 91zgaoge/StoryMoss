/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],
  theme: {
    extend: {
      colors: {
        parchment: '#f7f5ef',
        cream: '#fbf9f4',
        'terracotta-soft': '#f3e9e3',
        ink: '#2d2a26',
        charcoal: '#5a5550',
        stone: '#827c75',
        terracotta: '#b85c3e',
        'terracotta-dark': '#8f442c',
        gold: '#c9a35c',
        'ink-wash': 'rgba(45, 42, 38, 0.12)',
      },
      fontFamily: {
        display: ['"LXGW WenKai"', '"Source Han Serif CN"', '"Noto Serif SC"', 'serif'],
        body: ['"Source Han Serif CN"', '"Noto Serif SC"', '"Libre Baskerville"', 'serif'],
        sans: ['system-ui', '-apple-system', 'sans-serif'],
      },
      boxShadow: {
        cta: '0 8px 24px rgba(184, 92, 62, 0.2)',
        card: '0 2px 8px rgba(0, 0, 0, 0.04)',
        nav: '0 1px 3px rgba(0, 0, 0, 0.06)',
      },
      animation: {
        'ink-spread': 'inkSpread 0.6s ease-out forwards',
      },
      keyframes: {
        inkSpread: {
          '0%': { transform: 'scale(0)', opacity: '0.35' },
          '100%': { transform: 'scale(4)', opacity: '0' },
        },
      },
    },
  },
  plugins: [],
};
