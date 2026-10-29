# Lag, grenfester og konturredigering

Grunnlag: Siverts avklaring 29. oktober 2026 (lag, grunnrettinger, grenorientering, flytting av grenfeste, konturredigering, konturregister, og retning for Surface Pen og Surface Brush). Den har forrang foran [issue #4](https://github.com/sihovd/creature-forge/issues/4) og [planen for spline-skjelettet](spline_skjelett_plan.md) der de er uenige. Dokumentet skiller mellom det som er kontrollert i kode og tester, feil som er gjenskapt, problemer Sivert har rapportert, og funksjoner som ikke finnes ennå.

## 1 Kartlegging før trinn 2

Undersøkt på `main` etter PR #8 (commit 47ca4b2). Alle eksisterende tester gikk grønt: 58 Rust-tester og 20 nettlesertester (desktop og mobilprofil i Playwright).

| Område | Hva koden gjør | Status |
| --- | --- | --- |
| Splinehierarki | `core/src/skeleton.rs`: barn festet til en andel av forelderens buelengde, punktene lagret i festets lokale ramme. Validering avviser sløyfer og manglende foreldre. | Virker og er testet i kjernen. Hele hierarkiet regnes som ett lag, men laget har ingen egen representasjon (navn, skjul, lås). |
| Uttrekk og bøying | Dra tuppen, Alt panorerer, Alt + Shift bøyer med låst buelengde (`bend.rs`), scroll stiller radius, Ctrl snapper 15°. | Virker. Buelengden er testet i kjernen (avvik under 10⁻⁶) og i nettleseren (under 0,01 mm). |
| Bézier-redigering | Punkter og håndtak dras i synsplanet. Dobbeltklikk på kurven setter inn et punkt med de Casteljau-deling (formen beholdes). | Virker med mus. Dobbeltklikk finnes ikke på touch, så innsetting var utilgjengelig på iPad og telefon. |
| Konturer | `core/src/profile.rs`: lukkede 2D-Bézier-profiler, master og forekomster, overgang mellom naboprofiler. Redigeres i et lite felt i høyrepanelet. | Virker i panelet. Ingen redigering direkte på modellen, ingen plassering der brukeren trykker (unntatt høyreklikk), ikke valgbar i 3D. |
| Valg | Valg er en spline, eventuelt med ankerpunkt og kontur. | Ingen lagvalg, ingen liste, konturer og grener kunne bare velges ved å treffe punkter. |
| Mus, touch, penn | Pekerhendelser brukes overalt. Høyreklikk åpner hurtigmenyen. | Hurtigmenyen fantes ikke på touch (ingen langtrykk). |
| Angre og gjør om | Hver redigering lagrer hele skjelettet; uttrekk med prikkene spilles av på nytt. | Virker og er testet. |
| Lagring | Skjelettet kan serialiseres i kjernen. | Ingen lagring eller åpning av prosjektfil i appen. |

### Gjenskapte feil

1. **En gren i tuppen tar grepet om forelderens tupp.** Lag en spline, velg tuppen, trykk «Ny gren», og dra forelderens tupp: draget flytter grenens feste i stedet («Flytt festet langs forelderen»), og forelderen kan ikke trekkes ut. Årsak: grenens første punkt ligger på samme skjermpunkt, og ved lik avstand vant det siste punktet. Rettet i trinn 2 ved at grenfestet bare gripes når ingen andre punkter er like nær.
2. **«Ny spline» lages alltid loddrett** (`direction: [0, 1, 0]` i `main.ts`). Rettet i trinn 2 med aksekorset.
3. **Serialisering av tall var ikke tapsfri.** `serde_json` uten `float_roundtrip` kan endre siste siffer ved lesing, så en åpnet fil ga en litt annen mesh. Funnet med testen for prosjektfil i trinn 2 og rettet.

### Rapportert av Sivert, ikke gjenskapt

- At en ny barnespline skal ha egen uttrekksretning uavhengig av forelderens Bézier-håndtak. I dagens kode endrer verken «Ny gren» eller uttrekk av barnet forelderens kurve (kontrollert med nettlesertest), men retningen beregnes fra tangenten i nærmeste ankerpunkt, ikke i festepunktet. Rettes i trinn 3 sammen med retningshåndtaket for ny gren.

### Funksjoner som ikke fantes

Lagliste, navn, skjul og lås, aksekors, Bézier-verktøy for touch, langtrykk, orienteringsvalg for grener (følg forelder eller behold retning), flytting av grenfeste til en annen spline, konturredigering på modellen, konturregister, volumpresets og lagring til fil.

## 2 Rekkefølge

| Trinn | Innhold | Status |
| --- | --- | --- |
| 1 | Kartlegge og gjenskape feil | Gjort (dette dokumentet) |
| 2 | Lagliste, objektvalg, skjul og lås, aksekors for ny spline, Bézier-innsetting, langtrykk, lagring og åpning, fjerne «Legg til kule» | Gjort, se under |
| 3 | Orienteringsvalg for grener, «Ny gren» med retningshåndtak, flytting av festet gjennom spline-nettverket | Gjort, se under |
| 4 | «Ny kontur» der brukeren trykker, flytting langs splinen, konturredigering på modellen | Gjort, se under |
| 5 | Konturregister, presets (sirkel, ellipse, egg, trekant), volumpreset, masterkobling | Gjort, se under |
| 6 | Stabilisering før Surface Pen, Surface Brush og mirror | Ikke startet |

## 3 Trinn 2: lag og grunnrettinger

- **Lag.** Et lag er én rotspline med alle grener og undergrener. Lagets navn, skjuling og låsing lagres på rotsplinen (`name`, `hidden`, `locked`) og er en del av skjelettet, så de angres og lagres som andre endringer. Grener kan ha egne navn, men ikke skjules eller låses hver for seg. Masterkoblinger mellom konturer påvirker ikke laginndelingen.
- **Låsing håndheves i kjernen.** En redigering av en spline i et låst lag avvises (også inne i en samlet redigering), og det samme gjør uttrekk med prikkene. Navn, skjuling og låsing er tillatt, så laget kan låses opp. En masterendring avvises når en koblet kontur ligger i et låst lag.
- **Skjult og låst i visningen.** Et skjult lag tegnes ikke og treffes ikke. Et låst lag tegnes nedtonet og uten punkter; klikk går gjennom det. Høyreklikk eller langtrykk på et låst lag gir «Lås opp laget».
- **Laglisten** til venstre viser lag, hovedspline, grener og konturer som et tre som kan foldes ut og sammen. Et trykk velger hele laget, en spline eller en kontur; samme valg vises i 3D og i inspektøren til høyre (tittel og sti, for eksempel «Kontur 1 · Lag 1 / Hovedspline»). Dobbeltklikk eller navnefeltet i inspektøren gir nytt navn. På små skjermer ligger listen over arbeidsflaten og åpnes med «Lag».
- **Valg i 3D.** Et trykk på en kontur velger konturen, på en kurve splinen, på formen hele laget, og i tomt rom fjernes valget.
- **Ny spline.** «Ny spline» viser et aksekors med ±X, ±Y og ±Z. Brukeren velger uttrekksretning, og splinen lages i den retningen fra punktet på gulvet midt i bildet (eller der det ble høyreklikket). Valg av tegneplan er utsatt.
- **«Legg til kule» er fjernet** fra verktøylinjen og hurtigmenyen. Kjernens `addSphere` finnes fortsatt og brukes i tester.
- **Bézier-verktøyet.** Med «Bézier» setter et trykk på kurven inn et ankerpunkt der det ble trykket; formen beholdes. Et drag fra kurven roterer kameraet, og punkter og håndtak dras som før. Dobbeltklikk med «Velg» virker fortsatt. Håndtak vises bare for valgt ankerpunkt, så de ikke dekker kurven.
- **Langtrykk** med finger eller penn åpner samme hurtigmeny som høyreklikk med mus.
- **Lagring.** «Lagre» (Ctrl+S) laster ned `skapning.cfproj` med skjelett, lag, konturer, mastere og uttrekk. «Åpne» (Ctrl+O) leser filen; historikken starter fra filen. Formatet er JSON med `format` og `version`.

## 4 Trinn 3: grenens orientering og feste

- **Orientering** (`Spline::orientation`). «Følg forelder» lagrer grenens punkter i forelderens lokale ramme i festet, så vinkelen mot forelderen bevares når forelderen bøyes. «Behold retning» lagrer punktene med verdensaksene og bare festepunktet fra forelderen, så grenen flyttes uten å roteres; grenens egen referanseretning for tverrsnittet lagres også i verdensrommet, så profilene ikke vrir seg. Byttet regner om punktene fra den viste tilstanden, så grenen står der den står. Undergrener følger sine egne innstillinger. Valget ligger under «Gren» i inspektøren.
- **Ny gren** (knappen, eller «Ny gren her» i hurtigmenyen) viser først et retningshåndtak i festepunktet, foreslått vinkelrett på forelderens tangent i nettopp det punktet (ikke i nærmeste ankerpunkt). Håndtaket kan dras fritt for retning og lengde, et trykk godtar forslaget, og aksene ±X, ±Y og ±Z kan velges direkte. Forelderens kurve og håndtak endres ikke.
- **Flytting av festet.** Grenens første punkt dras langs forelderen. Nær et kryss (en gren festet til splinen, eller splinens eget feste) vises de andre splinene stiplet med en ring i krysset, og festet går over på den splinen pekeren er nærmest. Det gir en «vei gjennom veikrysset». Forelderkoblingen oppdateres i samme redigering (`Reattach` med `parent`), undergrenene følger med, og grenens punkter i festerammen beholdes: vinkelen mot den nye forelderen med «Følg forelder», verdensretningen med «Behold retning». Grenen selv og undergrenene dens er aldri mulige mål, og kjernen avviser feste til seg selv, til egne undergrener og sløyfer. Flyttingen er ett angresteg og lagres i prosjektfilen.

## 5 Trinn 4: konturplassering og konturredigering på modellen

- **Plassering.** «Ny kontur her» i hurtigmenyen (høyreklikk eller langt trykk på kurven eller formen) lager konturen der det ble trykket. Konturen kan gripes på ringen og dras fram og tilbake langs splinen; valget følger konturen selv om rekkefølgen langs splinen endres. Et vanlig trykk på ringen velger konturen, men åpner ikke redigeringen.
- **«Rediger kontur»** (hurtigmenyen på en kontur, eller knappen «Rediger kontur på modellen» i panelet) setter kameraet vinkelrett på konturplanet med profilens y-akse opp og zoomer inn. Modellen skjæres i konturplanet med et klippeplan, så tverrsnittet vises, og resten tegnes nedtonet og delvis gjennomsiktig. Omrisset redigeres i sitt eget plan oppå modellen: kjernen sender planets sentrum, normal og akser (`StationView::origin`, `normal`, `x_axis`, `y_axis`), og editoren legges nøyaktig over ringen i hvert bilde. Punkter og håndtak er større enn i panelet, så de kan brukes med finger og penn.
- **Verktøy i modusen:** dra punkter og håndtak, «Sett inn punkt» (et trykk på omrisset setter inn et punkt; dobbeltklikk virker også), «Hjørnepunkt»/«Mykt punkt», «Slett punkt», «Tegn nytt» og «Ferdig». Konkave profiler som H redigeres som andre profiler. Geometrien forhåndsvises under drag og lagres som ett angresteg når punktet slippes.
- **Kamera i modusen:** zoom og panorering virker, rotasjon er slått av så planet står vinkelrett. «Ferdig» eller Esc setter kameraet tilbake der det var. Skjelett, prikker og andre verktøy er inaktive så lenge modusen er åpen. På små skjermer skjules panelet, og verktøylinjen for konturen ligger nederst.
- **Masterkobling:** en koblet kontur redigeres gjennom masteren, og verktøylinjen viser «endrer masteren Mn og N konturer» med «Gjør individuell» ved siden av. Registeret og presetene kommer i trinn 5.

## 6 Trinn 5: konturregister, presets og masterprofiler

- **Registeret** ligger i skjelettet (`Skeleton::registry`) og lagres derfor i prosjektfilen og angres som andre endringer. En oppføring er enten én profil eller et volum (flere konturer med plass, størrelse og rotasjon langs splinen), alltid med egne omriss. «Lagre kontur» lagrer valgt kontur under et navn, og «Lagre volum» lagrer alle konturene på splinen. Oppføringer kan slettes. Registeret følger prosjektet, ikke nettleseren; et felles bibliotek på tvers av prosjekter er ikke laget.
- **Startpresets:** Sirkel, Ellipse, Egg og Trekant, i tillegg til Kvadrat og H fra før. Volumpresets: «Hode» (smal hals, større hodeparti med eggform og avsmalnende ende, fem konturer) og «Kule» (fem konturer som vokser og avtar etter splinens lengde). Alt bruker kontursystemet; det finnes ingen egne primitive meshobjekter.
- **Bruk:** en profil erstatter omrisset til valgt kontur, eller legges til som ny kontur når bare splinen er valgt. Et volum erstatter alle konturene på splinen (`SetStations`). Begge deler kopieres og kobles ikke varig, også når en koblet kontur får et preset (den blir individuell, masteren endres ikke).
- **Master:** «Gjør til master», «Koble til master» og «Rediger master» i panelet som før. En koblet kontur har eget omriss fra masteren, men egen størrelse, rotasjon og plass. Panelet og verktøylinjen i «Rediger kontur» viser hvor mange konturer en masterendring påvirker, og «Gjør individuell» bryter koblingen og beholder formen.

## 7 Videre retning som ikke implementeres nå

### Surface Pen og Surface Brush

Navnene er Surface Pen og Surface Brush (ikke «Surface Draw»). De bygges etter at lag, grenbetjening og konturarbeid fungerer.

- **Surface Pen.** En åpen strek lager en overflatespline. Et lukket omriss lager en egen overflateform. Både kanten og hele innsiden følger underlagets krumming.
- **Surface Brush.** Maler et område som blir faktisk, redigerbar geometri, for eksempel et plagg eller en nese. Formen kan flyttes langs underlaget og har egen tykkelse, offset og blending. Offset utover eller innover er noe annet enn å flytte området langs overflaten. Negative former skal ikke ødelegge sitt eget referanseunderlag.
- **Deformasjon og flytting.** En surface-form deformeres over hele festeområdet når underlaget endres; å flytte og rotere en uendret mesh er ikke nok. Eksempel: en nese malt på et ansikt tilpasses når ansiktet blir bredere og krummingen under nesen endres, og beholder egne detaljer, tykkelse, offset og blending. Flyttes nesen til øret, tilpasses festeområdet ørets krumming. Det krever både deformasjon med eksisterende underlag og ny binding ved flytting.
- **Hva koden støtter i dag.** Uttrekk med prikkene (`BoundPull`) lagres i rammen til nærmeste spline og spilles av på nytt. Det er en forelder–barn-transformasjon av ett grep, ikke en overflatebinding: det finnes ingen representasjon av et festeområde, ingen parametrisering av underlaget og ingen deformasjon av en egen form etter underlagets krumming. Surface-verktøyene krever en ny modell.
- **Lokal mesh-tetthet.** Oppløsningen tilpasses lokalt etter penselstørrelse og detaljbehov, uten at hele modellen fortettes. Detaljer og nødvendig oppløsning bevares når formen flyttes eller underlaget endres, og spline- og konturdefinisjonene endres ikke av fortettingen. I dag genereres kroppene med ett jevnt rutenett (`surface.rs`), og uttrekkene deler kanter lokalt (`refine.rs`). Eksportvalg som «Rask», «Normal» og «Fin» kan komme i tillegg, men må ikke endre form eller mekaniske klaringer.

### Farger og 3MF

Enkel, ensfarget tildeling til valgt lag, gren eller surface-form prioriteres når farger bygges: farge velges i inspektøren og er uavhengig av tykkelse, offset og geometrisk blending, og blending betyr ikke fargegradient. Del- og materialtilordning bevares med tanke på 3MF-eksport, og prosjektformatet beholder de redigerbare koblingene. Detaljmaling, teksturer og eksportsystemet er utsatt.

### Ikoner

Et sammenhengende startsett på omtrent 30 ikoner, blant annet spline, gren, kontur, lag, Surface Pen og Surface Brush, med tekst der symbolet alene er uklart. Retningen er myke, tydelige former med lilla og turkise aksenter. Sivert ønsker bildebaserte designutkast før vektorisering. Ikonsettet Sivert sendte 29. oktober 2026 (100 SVG-er) er referanse, ikke ferdige produksjonsikoner, og er ikke lagt i repoet.

### Utsatt

Mirror (til laginndelingen fungerer), frihåndstegning av barnesplines med penn eller finger, og valg av tegneplan for nye spliner.
