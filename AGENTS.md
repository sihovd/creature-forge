# Arbeidsregler for Creature Forge

Les docs/Creature_Forge_spesifikasjon.md før endringer. Dette er prosjektets produktgrunnlag. Nye direkte avklaringer fra Sivert har forrang; oppdater spesifikasjonen når en slik avklaring endrer kravene.

## Rollefordeling

Avklart med Sivert 27. oktober 2026:

- Claude er hovedutvikler og har hovedansvar for å bygge Creature Forge, implementere funksjoner og videreutvikle arkitekturen.
- ChatGPT/Codex har hovedansvar for uavhengig testing og kodegjennomgang: bygge og kjøre løsningen, kontrollere geometri og arbeidsflyt mot spesifikasjonen, gjenskape feil og dokumentere funn.
- ChatGPT/Codex kan gjøre målrettede feilrettinger og forbedringer ved behov. Større funksjonsutvikling og arkitekturomlegginger skal koordineres med Claude og Sivert.
- Sivert avgjør produktkrav og ønsket oppførsel.

Ved gjennomgang skal ChatGPT/Codex kontrollere den konkrete revisjonen, skille egne testresultater fra utviklerens påstander og oppgi hva som ikke er verifisert. Egne kodeendringer skal være avgrensede og sporbare, helst i en separat gren eller PR. Ikke overskriv pågående arbeid fra Claude eller slå sammen PR-er som del av en ren gjennomgang.

## Implementasjon

- Bygg reell funksjonalitet i den tiltenkte produksjonsarkitekturen. Ikke erstatt geometrikjernen med en visuell mockup.
- Ikke behandle foreslåtte teknologier, terskler eller ytelsesmål som godkjente eller oppnådde egenskaper.
- Ikke anta at kode fra Claude-artefakten oppfyller spesifikasjonen. Bevar importert kilde og registrer opprinnelse og eventuelle endringer.
- Sekundærformer skal deformeres geometrisk etter underlaget; flytting og rotasjon av en stiv mesh er utilstrekkelig.
- Alt panorerer grepet i planet vinkelrett på uttrekksretningen. Alt + Shift bøyer med låst buelengde, og scroll stiller bøyeradius (avklart 28. oktober 2026, erstatter låsen av rett avstand).
- Bevar varige senterlinjer og stabile overflatefester gjennom remeshing.
- Skill kroppsdelens fysiske volum, formens gruppe og dens overflateavhengighet.
- Separate bevegelige deler må ikke utilsiktet smeltes sammen.
- Endre ikke ShapeForge eller Designporten som del av dette repoet.

## Verifikasjon

Bruk relevante akseptansetester CF01 til CF25. Rapporter konkret hva som er kjørt, med resultat og kjente begrensninger. Ikke hev geometrisk eller fysisk validering på grunnlag av et skjermbilde.

Når kjørbar kode er lagt til, dokumenter faktiske installasjons-, bygge- og testkommandoer i README. Ikke oppgi kommandoer som ikke finnes eller er prøvd.

Ikke legg inn hemmeligheter eller bestem en åpen kildekodelisens uten en eksplisitt beslutning. Ikke publiser eller deploy appen uten at brukeren ber om det.
