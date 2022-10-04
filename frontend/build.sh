#!/bin/sh

export DIST="${out:-dist}"
mkdir -p "$DIST"
esbuild main.tsx >"$DIST/main.js"
cp index.html style.css favicon.ico "$DIST"
