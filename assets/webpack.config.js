module.exports = {
  mode: "none",
  entry: "./src/index.ts",
  output: {
    filename: "bundle.js",
    path: require("path").resolve(__dirname, "dist"),
  },
  module: {
    rules: [{ test: /\.tsx?$/, use: "ts-loader", exclude: /node_modules/ }],
  },
  resolve: { extensions: [".tsx", ".ts", ".js"] },
};
