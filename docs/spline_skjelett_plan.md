# Spline-skjelett: gjennomgang og implementeringsrekkefølge

Grunnlag: [issue #4](https://github.com/sihovd/creature-forge/issues/4), avklart med Sivert 28. oktober 2026. Dokumentet beskriver hva som finnes, hva som mangler og en avgrenset rekkefølge. Det påstår ikke at noe er implementert utover det som står under «Trinn 1» og er dekket av tester.

## Hva som finnes i dag

| Del | Status | Forhold til issue #4 |
| --- | --- | --- |
| `core/src/pull.rs` | Uttrekk fra et grep på en kule. Meshen deformeres langs én kubisk Hermite-kurve med lokal kantdeling. | Den direkte uttrekksmetoden som skal bevares. Lengden endrer seg i dag uforutsigbart når grepet flyttes. |
| `core/src/curve.rs` | Hermite-kurve med rotasjonsminimerende rammer. | Rammetransporten gjenbrukes. Ett segment per uttrekk, ingen Bézier-ankerpunkter. |
| `core/src/document.rs` | Operasjonslogg med angre og gjør om. Senterlinjen er parameterne til ett uttrekk. | Ingen hierarki, ingen fester mellom senterlinjer, ingen lagring til fil. |
| `web/src/pull-tool.ts` | Dra, Alt og Alt + Shift (låst rett avstand A–B). | Issue #4 endrer modifikatorene, se nedenfor. |
| Overflate | Én lukket trekantmesh per form. Ingen avstandsfelt, ingen boolske operasjoner. | Forgreninger som sammenhengende former krever en ny overflategenerering. |

Konklusjon: dagens senterlinje er en egenskap ved én meshdeformasjon. Issue #4 gjør senterlinjene til selve skjelettet som overflaten bygges rundt. Det krever en ny datamodell i kjernen, ikke en utvidelse av `Pull`.

## Endring i betjeningen

Issue #4 og Siverts avklaring 28. oktober 2026 er nyere enn spesifikasjonen kapittel 5 og har forrang. Issuen nevnte Shift for bøying; Sivert presiserte at det er Alt + Shift.

| Input | Før (kap. 5) | Nå (issue #4) |
| --- | --- | --- |
| Dra | Uttrekk langs normalen | Forleng langs uttrekksretningen |
| Alt + dra | Flytt i lokalt XY | Panorer: flytt ytterpunktet i planet vinkelrett på uttrekksretningen |
| Alt + Shift + dra | Lås rett avstand A–B | Bøying med låst buelengde, synlige håndtak og vinkelvisning. Erstatter låsen av rett avstand |
| Ctrl under bøying | – | Vinkelsnapping (15° eller 30°) |
| Scroll under bøying | – | Still størrelsen på kurven (bøyeradius) |

## Rekkefølge

### Grunnfunksjoner

**Trinn 1: Skjelettkjerne (denne PR-en).** Ren Rust med tester, ingen grensesnittendring.
- `core/src/skeleton.rs`: spliner av kubiske Bézier-segmenter med ankerpunkter, inn- og uthåndtak, myke punkter og hjørnepunkter.
- Hierarki: et barn er festet til en andel av forelderens **buelengde**. Barnets punkter lagres i festets lokale ramme. Dermed følger barn og undergrener når forelderen flyttes, bøyes eller redigeres med håndtak, og redigering av barnet endrer aldri forelderen. Festet er knyttet til kurven, ikke til mesh eller overflate.
- Tykkelse er en egen egenskap per spline og påvirker ikke festene.
- `core/src/bend.rs`: bøying med låst buelengde. Rett del, sirkelbue og rett rest summerer alltid til S. Vinkel 0–180° i en vilkårlig retning, slik at enderetningen kan ligge hvor som helst på en kule. Radius begrenses til S/θ, og radius null gir en eksplisitt knekk. Snapping er en egen funksjon. En bøy er høyst 180°; for å fortsette legges et nytt punkt inn i senterlinjen og resten bøyes derfra med låst lengde (`bend_from`). Bøyen starter på midten av kurven, og scroll utvider den like mye til begge sider (`DEFAULT_PIVOT`). Begge deler er avklart med Sivert 28. oktober 2026.
- Lagring som JSON med format og versjon, med kontroll av fester, sløyfer og tall ved åpning.

**Trinn 2: Skjelettet i appen.** Wasm-grensesnitt, visning av ankerpunkter og håndtak, legge til rot- og barnespliner, angre og gjør om, og lagring/åpning av prosjektfil (overlapper milepæl 2). Hele hierarkiet er ett formlag.

**Trinn 3: Kontrollert uttrekk og bøying.** Dra, Alt, Alt + Shift, Ctrl og scroll som i tabellen over, med visning av vinkel, retning og bøyeplan. Bøyen starter på midten av kurven, og scroll utvider den.

**Trinn 4: Overflate fra skjelettet med sirkel og ellipse.** Hver spline får tverrsnitt langs kurven. For at forgreninger skal bli sammenhengende former og ikke overlappende rør, foreslås et avstandsfelt rundt hver spline (feid tverrsnitt) med glatt sammensmelting, og en overflate hentet ut av feltet (for eksempel dual contouring eller surface nets i Rust). Dagens kjerne har ingen slik representasjon, så dette er en ny del som må måles for ytelse og detaljnivå. Endeprofilen bevares helt ut; en spiss er tilfellet der profilen går til null.

### Videre profilredigering

**Trinn 5: Profiler som data.** Lukkede 2D-Bézier-profiler, master og forekomster med egen størrelse, rotasjon og posisjon. Gjør til master, koble til, gjør individuell (beholder gjeldende form). Kopier og lim inn langs senterlinjen. Overgang mellom naboprofiler med samplet korrespondanse og valgt startpunkt som gir minst vridning.

**Trinn 6: Profileditor i grensesnittet.** Tegne nytt omriss, legge til punkter, hjørner og håndtak, konkave profiler som H. Tydelig skille mellom senterlinjeredigering og tverrsnittsredigering, og en eksplisitt handling for å redigere masteren.

Surface tools er et eget verktøysystem og inngår ikke.

## Åpne spørsmål til Sivert

Ingen på nåværende tidspunkt. Pivot, grense på 180° og Alt + Shift er avklart 28. oktober 2026.

## Verifikasjon i trinn 1

Kjørt med `cargo test` i denne PR-en:

- Flytting og bøying av en forelder med tre nivåer: barn og barnebarn følger, barnets lengde og vinkel mot forelderen er uendret.
- Redigering av barnet (punkt, håndtak, punkttype, feste langs forelderen) endrer ikke forelderens punkter.
- Tykkelsesendring endrer ingen fester.
- Bézier-håndtak på forelderen: mykt punkt retter inn motsatt håndtak, hjørnepunkt holder dem uavhengige, og hierarkiet består.
- Buelengde ved 0–180°, radius 0–1000 mm, flere pivoter og snappet vinkel: største relative avvik målt til 1,9·10⁻⁷, krav i testen er 10⁻⁶ (`LENGTH_TOLERANCE`).
- 90° gir enderetning vinkelrett, 180° vender enden tilbake.
- Bøyen sentreres på midten av kurven og vokser like mye til begge sider når radien økes.
- Nytt punkt i bøyen og to bøyer på 180° etter hverandre gir 360° med uendret lengde og uendret del før punktet.
- Radius null gir en knekk med gyldig senterlinje og ortonormale rammer gjennom knekken, også ved 180°.
- Lagring og åpning gir likt skjelett. Ukjent format, manglende forelder og sløyfer avvises.

Ikke verifisert: noe i nettleseren, overflate rundt skjelettet, profiler og fysisk utskrift.
