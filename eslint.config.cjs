module.exports = [
  {
    files: ["app/ui/*.js"],
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: "script",
      globals: Object.fromEntries(
        [
          "window",
          "document",
          "navigator",
          "location",
          "localStorage",
          "matchMedia",
          "crypto",
          "URL",
          "Audio",
          "setTimeout",
          "clearTimeout",
          "console",
          "structuredClone",
        ].map((name) => [name, "readonly"]),
      ),
    },
    rules: {
      "no-undef": "error",
      "no-unreachable": "error",
      "no-constant-condition": "error",
    },
  },
];
