#!/usr/bin/env bash

esbuild index.tsx --bundle --minify --sourcemap --target=esnext --outfile=dist/index.js
cp index.html dist/index.html
cp favicon.ico dist/favicon.ico
