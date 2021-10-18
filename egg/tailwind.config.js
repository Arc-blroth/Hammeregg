module.exports = {
  mode: 'jit',
  purge: ['./src/*.{html,js}'],
  darkMode: false,
  theme: {
    // see https://github.com/emilk/egui/blob/master/egui/src/style.rs#L404
    extend: {
      colors: {
          gray: {
            '100': '#373737',
            '200': '#3c3c3c',
            '500': '#1b1b1b',
            '600': '#696969',
            '700': '#0a0a0a',
          },
          ringblue: '#c0deff',
      },
      textColor: {
        primary: '#8b8b8b',
        secondary: '#aaaaaa',
        button: '#a8a8a8',
      },
      fontSize: {
        tiny: '0.875rem',
        base: '1rem',
        lg: '1.5rem',
        xl: '2rem',
      },
      lineHeight: {
        'extra-tight': '0',
      }
    },
  },
  variants: {
      extend: {},
  },
  plugins: [],
}