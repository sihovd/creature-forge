# creature-forge

Nettleserbasert 3D-modellering av artikulerte figurer med overflatefølgende former og egne ledd.

## Oppbygging

| Mappe | Innhold |
| --- | --- |
| `core/` | Geometrikjernen i Rust, kompilert til WebAssembly. Alle lengder er i millimeter. |
| `web/` | Nettappen: TypeScript, Vite og Three.js. Kjernen kjører i en Web Worker. |

Appen starter alltid med et tomt arbeidsrom. Brukeren legger selv til første form.

## Kom i gang

Krever Rust (med `wasm32-unknown-unknown`), [wasm-pack](https://rustwasm.github.io/wasm-pack/) og Node 22.

```sh
cargo test               # tester for geometrikjernen
cd web
npm install
npm run core             # bygger Rust-kjernen til web/src/core-pkg
npm run dev              # utviklingsserver på http://localhost:5173
npm run build && npm run test:e2e   # nettlesertester (desktop og mobil)
```
