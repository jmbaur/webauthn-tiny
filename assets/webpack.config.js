const path = require("path");
const HtmlWebpackPlugin = require("html-webpack-plugin");
const CopyPlugin = require("copy-webpack-plugin");

module.exports = {
  mode: "none",
  entry: "./src/index.ts",
  devtool: "inline-source-map",
  output: {
    filename: "index.js",
    path: path.resolve(__dirname, "dist"),
  },
  module: {
    rules: [{ test: /\.tsx?$/, use: "ts-loader", exclude: /node_modules/ }],
  },
  plugins: [
    new HtmlWebpackPlugin({ title: "WebAuthnTiny", favicon: "favicon.ico" }),
    new CopyPlugin({
      patterns: ["favicon.ico"],
    }),
  ],
  resolve: { extensions: [".tsx", ".ts", ".js"] },
};
