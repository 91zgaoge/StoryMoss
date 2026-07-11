/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],
  theme: {
    extend: {
      colors: {
        parchment: '#f8f6f1',
        cream: '#fbf9f4',
        ink: '#1a1816',
        charcoal: '#6b6560',
        stone: '#827c75',
        cinnabar: '#a83f2e',
        'cinnabar-dark': '#7d2e21',
        'ink-line': '#e3ded4',
        'ink-wash': 'rgba(26, 24, 22, 0.03)',
      },
      fontFamily: {
        display: ['"LXGW WenKai"', '"Source Han Serif CN"', '"Noto Serif SC"', 'serif'],
        body: ['system-ui', '-apple-system', '"PingFang SC"', '"Microsoft YaHei"', 'sans-serif'],
      },
      boxShadow: {
        cta: '0 8px 24px rgba(168, 63, 46, 0.18)',
        card: '0 1px 3px rgba(0, 0, 0, 0.04)',
        nav: '0 1px 2px rgba(0, 0, 0, 0.04)',
      },
    },
  },
  plugins: [],
};
