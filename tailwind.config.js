module.exports = {
  purge: [
      "./templates/**/*.html",
      "./src/**/*.rs",
      "./styles/tailwind.css",
  ],
  darkMode: false, // or 'media' or 'class'
  theme: {
    extend: {},
  },
  variants: {
    extend: {},
  },
  plugins: [],
}
