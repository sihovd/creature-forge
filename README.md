# Creature Forge

Nettleserbasert 3D-modellering av artikulerte figurer for 3D-print, med forming fra en kule, redigerbare senterlinjer, overflatefølgende sekundærformer og en egen hengseleditor.

## Prosjektgrunnlag

Les [produktdefinisjonen og den tekniske funksjonsspesifikasjonen](docs/Creature_Forge_spesifikasjon.md) før implementasjon. Gjeldende versjon er 1.1 fra 27. oktober 2026.

Spesifikasjonen skiller mellom avklarte krav, tidligere produktretning, tekniske forslag og egenskaper som må verifiseres. Den inneholder 25 akseptansetester. Forslag til biblioteker og ytelsesmål er ikke ferdig validerte valg.

## Rollefordeling

Claude er hovedutvikler og bygger appen. ChatGPT/Codex tester og gjennomgår arbeidet uavhengig, kontrollerer det mot spesifikasjonen og gjør målrettede feilrettinger eller forbedringer ved behov. Større utviklingsarbeid koordineres med Claude og Sivert. Sivert avgjør produktkravene.

Arbeidsmåten er beskrevet i [AGENTS.md](AGENTS.md).

## Status

Milepæl 1 ([PR #1](https://github.com/sihovd/creature-forge/pull/1)) legger inn Rust/Wasm-kjerne, worker, Three.js-grensesnitt og tomt arbeidsrom ved oppstart. Den dekker ennå ingen av akseptansetestene CF01–CF25; den er en motorprøve for arkitekturen.

Formgrep er på plass: kontrollpunkter fordelt etter punktavstand, pil og påvirkningsområde ved hover, uttrekk og innpressing ved drag med lokal kantdeling, Escape for å avbryte, og angre og gjør om der hvert drag er ett steg. Nettlesertestene dekker deler av CF01–CF03, CF06 og CF21 med mus på desktop; berøring er ikke automatisk testet ennå. Alt-modusene, varige senterlinjer, lagring og et ferdig grensesnitt gjenstår.

Sivert avklarte 27. oktober 2026 at brukeren skal se et tomt arbeidsrom når appen starter. Spesifikasjonen (kapittel 3, steg 1) er oppdatert tilsvarende.

Claude-artefakten er lest og eksportert. Den inneholder en utviklingsplan, ikke applikasjonskode. Originaleksporten er bevart uendret i [Claude-planen](docs/references/Claude_plan_for_forste_versjon_original.md).

Kilde: https://claude.ai/artifact/XWFCDBhgc7gS5yg2xB1vqn#1aad0e36-3736

Planen er referansemateriale. Foreslått teknologistakk og redusert omfang for første milepæl er ikke behandlet som godkjente beslutninger. Publisering til GitHub Pages fra `main` er satt opp etter at Sivert ba om at repoet skulle gjøres offentlig og publiseres.

## Oppbygging

| Mappe | Innhold |
| --- | --- |
| `core/` | Geometrikjernen i Rust, kompilert til WebAssembly. Alle lengder er i millimeter. |
| `web/` | Nettappen: TypeScript, Vite og Three.js. Kjernen kjører i en Web Worker. |
| `docs/` | Spesifikasjon og referansemateriale. |

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

## Sentrale krav

- Trekk ut et kontrollpunkt eller et markert meshområde og bøy formen.
- Alt + Shift låser avstanden mellom startpunkt og grep. Senterlinjen beholdes for videre redigering.
- Oppdater topologien dynamisk ved både utvidelse, bøyning og krymping.
- Deformer sekundærformens egen geometri etter underlagets fasong og krumming.
- Hold tykkelse, offset, avrunding og blending som separate egenskaper.
- Rediger egne hengsletyper med målrettede negative former og dokumentert klaringskontroll.
- Bevar redigerbar konstruksjon gjennom lagring og eksport.

Prosjektet starter fra bunnen av og forutsetter ikke ShapeForge eller eksisterende Designporten-kode.
