# Creature Forge

Nettleserbasert 3D-modellering av artikulerte figurer for 3D-print, med forming fra en kule, redigerbare senterlinjer, overflatefølgende sekundærformer og en egen hengseleditor.

## Prosjektgrunnlag

Les [produktdefinisjonen og den tekniske funksjonsspesifikasjonen](docs/Creature_Forge_spesifikasjon.md) før implementasjon. Gjeldende versjon er 1.1 fra 27. oktober 2026.

Spesifikasjonen skiller mellom avklarte krav, tidligere produktretning, tekniske forslag og egenskaper som må verifiseres. Den inneholder 25 akseptansetester. Forslag til biblioteker og ytelsesmål er ikke ferdig validerte valg.

## Rollefordeling

Claude er hovedutvikler og bygger appen. ChatGPT/Codex tester og gjennomgår arbeidet uavhengig, kontrollerer det mot spesifikasjonen og gjør målrettede feilrettinger eller forbedringer ved behov. Større utviklingsarbeid koordineres med Claude og Sivert. Sivert avgjør produktkravene.

Arbeidsmåten er beskrevet i [AGENTS.md](AGENTS.md).

## Status

På `main` ligger prosjektgrunnlaget og referanseplanen. Første kodeforslag finnes i [PR #1: Milepæl 1](https://github.com/sihovd/creature-forge/pull/1), på grenen `milepael-1-oppsett`. PR-en beskriver Rust/Wasm-kjerne, worker, Three.js-grensesnitt og tomt arbeidsrom ved oppstart. Den er ikke slått sammen med `main`.

Testresultater og ytelsestall i PR-beskrivelsen er rapportert av forfatteren og er ikke uavhengig verifisert ved denne dokumentimporten. Bygge- og testinstruksjoner for applikasjonen må leses sammen med kodeforslaget.

Claude-artefakten er lest og eksportert. Den inneholder en utviklingsplan, ikke applikasjonskode. Originaleksporten er bevart uendret i [Claude-planen](docs/references/Claude_plan_for_forste_versjon_original.md).

Kilde: https://claude.ai/artifact/XWFCDBhgc7gS5yg2xB1vqn#1aad0e36-3736

Planen er referansemateriale. Foreslått teknologistakk, redusert omfang for første milepæl og automatisk publisering er ikke behandlet som godkjente beslutninger eller implementert. Planen oppgir også tomt arbeidsrom ved oppstart, mens spesifikasjon 1.1 viser en kule i arbeidsflyten; dette avviket er registrert og må avklares før implementasjon av oppstarten. Spesifikasjonen er ikke endret på grunnlag av denne importen.

## Sentrale krav

- Trekk ut et kontrollpunkt eller et markert meshområde og bøy formen.
- Alt + Shift låser avstanden mellom startpunkt og grep. Senterlinjen beholdes for videre redigering.
- Oppdater topologien dynamisk ved både utvidelse, bøyning og krymping.
- Deformer sekundærformens egen geometri etter underlagets fasong og krumming.
- Hold tykkelse, offset, avrunding og blending som separate egenskaper.
- Rediger egne hengsletyper med målrettede negative former og dokumentert klaringskontroll.
- Bevar redigerbar konstruksjon gjennom lagring og eksport.

Prosjektet starter fra bunnen av og forutsetter ikke ShapeForge eller eksisterende Designporten-kode.
