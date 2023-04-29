This game can be built for browsers. The game code can be compiled by targeting
wasm32-unknown-emscripten:

```sh
cargo build --release --target wasm32-unknown-emscripten
```

Then collect the following files into one directory:

- `target/wasm32-unknown-emscripten/release/sailing_venus.wasm`
- `target/wasm32-unknown-emscripten/release/sailing-venus.js`
- `resources/web/index.html`

Point a web server at that directory, and there you go. The HTML file is built
for embedding into an iframe, so it may look somewhat sparse if opened in a
browser directly.
