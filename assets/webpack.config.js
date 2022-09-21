const HtmlWebpackPlugin = require("html-webpack-plugin");
const FaviconsWebpackPlugin = require("favicons-webpack-plugin");

module.exports = {
  mode: "none",
  entry: "./src/index.ts",
  devtool: "inline-source-map",
  output: {
    filename: "bundle.js",
    path: require("path").resolve(__dirname, "dist"),
  },
  module: {
    rules: [{ test: /\.tsx?$/, use: "ts-loader", exclude: /node_modules/ }],
  },
  plugins: [
    new HtmlWebpackPlugin({ title: "WebAuthnTiny" }),
    new FaviconsWebpackPlugin("favicon.ico"),
  ],
  resolve: { extensions: [".tsx", ".ts", ".js"] },
};
