/** @type {import('tailwindcss').Config} */
export default {
  content: ['./index.html', './src/**/*.{js,ts,jsx,tsx}'],
  theme: {
    extend: {
      colors: {
        // 墨苔书斋（dark ink-green study）
        canvas: 'oklch(0.175 0.022 158)',
        'canvas-2': 'oklch(0.21 0.025 158)',
        moss: 'oklch(0.76 0.13 148)',
        'moss-deep': 'oklch(0.58 0.11 152)',
        'moss-soft': 'oklch(0.86 0.08 148)',
        paper: 'oklch(0.93 0.012 130)',
        mist: 'oklch(0.75 0.025 150)',
        dim: 'oklch(0.6 0.02 155)',
      },
      fontFamily: {
        display: ['"LXGW WenKai"', '"Source Han Serif CN"', '"Noto Serif SC"', 'serif'],
        body: ['system-ui', '-apple-system', '"PingFang SC"', '"Microsoft YaHei"', 'sans-serif'],
      },
      borderRadius: {
        sm: '6px',
        md: '10px',
        lg: '16px',
      },
      letterSpacing: {
        display: '-0.022em',
        mid: '-0.012em',
      },
    },
  },
  plugins: [],
};
