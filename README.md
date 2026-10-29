# Creature Forge

Nettleserbasert 3D-modellering av artikulerte figurer for 3D-print, med forming fra en kule, redigerbare senterlinjer, overflatefølgende sekundærformer og en egen hengseleditor.

## Prosjektgrunnlag

Les [produktdefinisjonen og den tekniske funksjonsspesifikasjonen](docs/Creature_Forge_spesifikasjon.md) før implementasjon. Gjeldende versjon er 1.2 fra 28. oktober 2026.

Spesifikasjonen skiller mellom avklarte krav, tidligere produktretning, tekniske forslag og egenskaper som må verifiseres. Den inneholder 25 akseptansetester. Forslag til biblioteker og ytelsesmål er ikke ferdig validerte valg.

## Rollefordeling

Claude er hovedutvikler og bygger appen. ChatGPT/Codex tester og gjennomgår arbeidet uavhengig, kontrollerer det mot spesifikasjonen og gjør målrettede feilrettinger eller forbedringer ved behov. Større utviklingsarbeid koordineres med Claude og Sivert. Sivert avgjør produktkravene.

Arbeidsmåten er beskrevet i [AGENTS.md](AGENTS.md).

## Status

Milepæl 1 ([PR #1](https://github.com/sihovd/creature-forge/pull/1)) legger inn Rust/Wasm-kjerne, worker, Three.js-grensesnitt og tomt arbeidsrom ved oppstart. Den dekker ennå ingen av akseptansetestene CF01–CF25; den er en motorprøve for arkitekturen.

Formgrep er på plass: kontrollpunkter fordelt etter punktavstand, pil og påvirkningsområde ved hover, uttrekk og innpressing ved drag med lokal kantdeling, Escape for å avbryte, og angre og gjør om der hvert drag er ett steg. Nettlesertestene dekker deler av CF01–CF03, CF06 og CF21 med mus på desktop; berøring er ikke automatisk testet ennå.

Mesh rundt splinene: Sivert avklarte 29. oktober 2026 at det alltid skal være mesh rundt en spline, at kula også har en spline i midten, og at deformering med prikkene ikke skal lage spliner. Hvert spline-hierarki er nå én kropp med mesh generert rundt splinene fra et avstandsfelt, der grenene smelter glatt sammen med forelderen. Prikkene trekker ut og presser inn meshen som i første utgave; uttrekkene spilles av på nytt oppå meshen når skjelettet endres. Bøying og forlengelse gjøres med splinene.

Spline-skjelett ([issue #4](https://github.com/sihovd/creature-forge/issues/4)): kjernen har fått et skjelett av Bézier-spliner der barn er festet til forelderens kurve, og bøying med låst buelengde. Skjelettet er koblet til appen: «Ny spline» og «Ny gren», punkter og håndtak dras i synsplanet, tuppen trekkes ut, Alt panorerer, Alt + Shift bøyer med låst buelengde fra valgt punkt, scroll stiller radien og Ctrl snapper til 15°. Dobbeltklikk legger inn et punkt, og grenenes feste dras langs forelderen. Hvert drag er ett angresteg. Grip kurven til en rotspline for å flytte hele formen, og et punkt som dras nær en annen spline, snapper til den (Shift slår av). Konturer (tverrsnitt) kan legges langs splinen: sirkel, ellipse, kvadrat, H eller et eget tegnet omriss, med egne punkter, håndtak og hjørner, kopier og lim inn, størrelse og rotasjon, og masterprofiler som oppdaterer alle koblede konturer. Uttrekk med prikkene følger formen når den flyttes eller bøyes. Høyreklikk gir hurtigvalg. Gjennomgang, rekkefølge og åpne spørsmål står i [planen for spline-skjelettet](docs/spline_skjelett_plan.md).

Lag og grunnrettinger (29. oktober 2026): et lag er én rotspline med alle grener. Laglisten til venstre viser lag, grener og konturer; valg i listen og i 3D er det samme, og lag kan navngis, skjules og låses (låsing håndheves i kjernen). «Ny spline» viser et aksekors der uttrekksretningen velges, og «Legg til kule» er fjernet. Bézier-verktøyet setter inn et ankerpunkt der brukeren trykker på kurven. Langt trykk med finger eller penn gir samme hurtigmeny som høyreklikk. «Lagre» og «Åpne» bruker prosjektfilen `.cfproj`. En gren kan følge forelderens rotasjon eller beholde retningen i rommet, «Ny gren» viser et retningshåndtak, og grenens feste kan dras langs forelderen og gjennom kryss over på andre spliner i laget. «Rediger kontur» viser tverrsnittet med kameraet vinkelrett på konturen, og omrisset redigeres direkte på modellen; konturer kan dras langs splinen. Kartlegging, rekkefølge og videre retning (Surface Pen og Surface Brush, farger, ikoner) står i [lag, grenfester og konturredigering](docs/lag_og_konturer_plan.md).

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
npm test                 # enhetstester for grepsberegningene
npm run build && npm run test:e2e   # nettlesertester (desktop og mobil)
```

## Sentrale krav

- Trekk ut et kontrollpunkt eller et markert meshområde og bøy formen.
- Alt panorerer grepet. Alt + Shift bøyer med låst buelengde, og scroll stiller bøyeradius. Senterlinjen beholdes for videre redigering.
- Oppdater topologien dynamisk ved både utvidelse, bøyning og krymping.
- Deformer sekundærformens egen geometri etter underlagets fasong og krumming.
- Hold tykkelse, offset, avrunding og blending som separate egenskaper.
- Rediger egne hengsletyper med målrettede negative former og dokumentert klaringskontroll.
- Bevar redigerbar konstruksjon gjennom lagring og eksport.

Prosjektet starter fra bunnen av og forutsetter ikke ShapeForge eller eksisterende Designporten-kode.
