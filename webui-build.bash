#!/usr/bin/env bash

esbuild index.tsx --bundle --minify --sourcemap --target=es2020,chrome58,edge18,firefox57,node12,safari11 --outfile=dist/index.js
cp index.html dist/index.html
cp favicon.ico dist/favicon.ico
