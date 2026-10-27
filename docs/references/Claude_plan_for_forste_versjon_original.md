# Creature Forge – plan for første versjon

Oct 27, 2026 · @Sivert hovde

## Hva appen skal gjøre

Creature Forge er et modelleringsprogram i nettleseren (PC, Mac og mobil) for å lage artikulerte figurer som printes ferdig sammen med bevegelige ledd.

- **Forming:** Brukeren starter med en kule, drar ut kontrollpunkter eller markerte områder, og bøyer uttrekket med Alt (flytt i plan) eller Alt + Shift (låst trådlengde). Hvert uttrekk får en varig senterlinje som kan redigeres senere, og meshen remeshes underveis.
- **Lag:** Med penneverktøyet tegnes lukkede omriss på overflaten, som trekkes ut med egen tykkelse, offset, avrunding og blending. Former kan ha underformer og flyttes langs overflaten.
- **Ledd:** Koblingspunkter plasseres på kroppsdeler, og en egen hengseleditor lager hengsletyper med negative former som skjærer ut plass til bevegelse. Klaring og bevegelse kontrolleres.
- **Ut av appen:** Redigerbar prosjektfil (.cfproj), autolagring i nettleseren, og eksport til STL og 3MF med kontrollrapport.

Spesifikasjonen er tydelig på at dette skal være en ekte geometrimotor fra start, ikke en visuell demo. Den deler arbeidet i fem trinn, og første trinn er dokumentmodell og formgrep.

## Foreslått teknologi

Jeg anbefaler det spesifikasjonen selv foreslår: TypeScript og Three.js i nettleseren, med en geometrikjerne i Rust kompilert til WebAssembly og kjørt i en Web Worker. Alt kjører lokalt, så det trengs ingen server.

| Del | Valg | Hvorfor |
| --- | --- | --- |
| Grensesnitt | TypeScript, Vite, React for paneler | Raskt oppsett, gode typer, enkelt å lage sidepanel og skuffer for mobil |
| 3D-visning | Three.js med WebGL 2 først | Moden og godt dokumentert; WebGPU kan slås på senere uten å endre kjernen |
| Geometrikjerne | Rust + wasm-bindgen, i Web Worker | Rask nok for remeshing, gode tester med cargo test, UI fryser ikke |
| Boolske operasjoner (senere) | Manifold (manifold-3d, Apache-2.0) | Robuste sammenføyninger og utskjæringer, har ferdig Wasm-bygg; CGAL unngås pga. GPL |
| Lagring | IndexedDB for autolagring, .cfproj (zip) for nedlasting | Som i spesifikasjonen kap. 19 |
| Tester | cargo test, Vitest, Playwright | Geometri måles i Rust; CF-testene kjøres i ekte nettleser |
| Publisering | GitHub Pages eller Cloudflare Pages | Statisk side, gratis, ny forhåndsvisning per PR |

Den største risikoen er Rust/Wasm-koblingen: to språk og datakopiering mellom worker og visning. Derfor måles den i aller første milepæl, før noe annet bygges på den.

## Plan for første versjon

Første versjon er spesifikasjonens trinn 1, pluss enkel STL-eksport: én kroppsdel som kan formes med grep, bøyes med Alt-modusene, redigeres via senterlinjen, lagres, angres og printes. Hver milepæl blir en egen PR med tester og en lenke du kan prøve i nettleseren.

**Krav fra Sivert (27. oktober):** Brukeren skal se et tomt arbeidsrom når appen starter. Ingen kule eller eksempelmodell lastes automatisk; første form legges til med et tydelig valg, for eksempel «Legg til kule». Dette gjelder også nytt prosjekt, selv om spesifikasjonen (kap. 3, steg 1) sier at en kule vises med en gang.

1. **Oppsett.** Repo med `web/` (TypeScript) og `core/` (Rust), automatiske tester og publisering ved hver PR.
2. **Motorprøve.** Appen starter med et tomt arbeidsrom (rutenett og kamera, ingen modell). Brukeren legger selv til en kule, som Rust-kjernen lager i workeren og Three.js viser. Måle tid og datakopiering, slik spesifikasjonen krever før arkitekturen låses.
3. **Dokument, angre og lagring.** Prosjektmodell med stabile ID-er og millimeter, transaksjoner (ett drag = ett angresteg), autolagring i IndexedDB og nedlasting/åpning av .cfproj.
4. **Kontrollpunkter og uttrekk.** Jevnt fordelte grep med justerbar avstand, pil ved hover eller trykk, påvirkningsradius, uttrekk og innpressing av punkt og av markert område med pensel (CF01–CF03).
5. **Alt og Alt + Shift.** Flytt i plan og låst trådlengde etter formlene i kap. 24, uten hopp ved modusbytte, Escape avbryter. Samme moduser som synlige knapper for mobil (CF04–CF06).
6. **Varig senterlinje.** Hermite-kurve med rotasjonsminimerende rammer; formen følger når linjen redigeres. Enkel lokal kantdeling så trekantene ikke strekkes (CF07–CF08).
7. **Eksport og avslutning.** STL-eksport i millimeter av gjeldende form, lagre-og-åpne-test på annen enhet, og en liste over kjente begrensninger (CF21–CF22).

Full remeshing med stabile fester (trinn 2) kommer rett etter, før lag og pennverktøy, slik spesifikasjonen anbefaler.

## Utenfor første versjon

Pennverktøy og lag, underformer, speiling, koblingspunkter, hengseleditor, negative former, sammenstilling av flere deler, 3MF og kontrollrapport venter til trinn 2–5. Kontoer, betaling og AI er utenfor hele produktet slik spesifikasjonen er skrevet.

## Spørsmål til Sivert før start

- [ ] Opprett et tomt repo på GitHub, for eksempel `sihovd/creature-forge`, og si fra. Da kobler jeg det til og starter med milepæl 1.
- [ ] Er det greit at første versjon bare dekker én kroppsdel (forming, bøying, senterlinje, lagring, STL), og ikke en hel figur med ledd?
- [ ] Alt-modus: skal «XY» være et plan vinkelrett på uttrekksretningen (spesifikasjonens forslag), skjermplanet eller globalt XY?
- [ ] Er GitHub Pages greit som publiseringssted, eller har du et annet sted du vil ha appen?
