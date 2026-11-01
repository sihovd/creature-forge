# Issue #12: arbeidsplan, splinefyll, negativlag, ringgrep og Surface Brush

Plan og status for issue #12 (Sivert, 1. november 2026). Bygger på PR #10
(nett av spliner, knekkfortetting, mesh-deler, speil om x = 0), som ikke er
slått sammen ennå. Dette arbeidet ligger derfor i en gren oppå PR #10, ikke
parallelt med den.

## Status per punkt

| # | Punkt | Status |
|---|-------|--------|
| 1 | Arbeidsplan i stedet for uttrekksretning | Ikke startet. Åpne spørsmål under. |
| 2 | Fyll lukket splinebane | Ikke startet. Åpne spørsmål under. |
| 3 | Mirror Tool (world og lokal) | World finnes fra PR #10, og kutt speiles nå også. Lokal speiling ikke startet (ikke definert). |
| 4 | Negativ form i lageditoren | **Trinn 1, implementert i denne grenen.** |
| 5 | Lokal fortetting etter vinkel | Finnes fra PR #10. Målt under; ett funn. |
| 6 | Skaler konturringen i arbeidsrommet | Ikke startet. Neste foreslåtte trinn. |
| 7 | Meshforming oppdaterer konturen | Ikke startet. Krever kartlegging (se under). |
| 8 | Surface Brush | Ikke startet. Største punkt; egne trinn. |

## Trinn 1: negativ form (punkt 4)

Implementert:

- Et lag kan være en negativ form (`Spline.cut` på rotsplinen, redigeringen
  `SetCut`). Lagraden har en knapp «＋»/«−» som bytter mellom «Legg til» og
  «Trekk fra», og en negativ form har minusmerke med antall mål («− 2»).
- Kuttemålene er en eksplisitt liste over lag. Når et lag gjøres om, foreslås
  lagene det overlapper (med speilbildet i speilmodus). Listen vises og
  endres i inspektøren under «Kutter i». Forelder og feste er en annen
  relasjon: en negativ form kan bare være et helt lag, og målene er lag-id-er,
  så rekkefølgen i laglisten endrer ikke hva som kuttes.
- «Utskjæring aktiv» og «Vis kutter» er separate. «Vis kutter» er lagets
  synlighet; en skjult kutter skjærer fortsatt, og en synlig kutter med
  utskjæringen av skjærer ikke.
- «Skarp kant» eller myk overgang mot kuttet.
- Kutteren tegnes gjennomsiktig lilla og har ingen kontrollpunkter (den
  formes med skjelettet, og kuttet følger splinene). Den skjuler ikke
  kontrollpunktene på målet bak seg.
- Kuttet er reell geometri: kutterens avstandsfelt trekkes fra målets felt
  (glatt maksimum) før overflaten hentes ut, så meshen er lukket. Feltet i
  enkeltpunkter (brukt av knekkfortettingen) har samme kutt, så fortettingen
  legger punkter på den kuttede flaten.
- Skjelettet og festene i målet berøres ikke. En gren som sitter der
  materialet fjernes, er fortsatt festet.
- Slettes et mål, fjernes det fra kutterens liste. En kutter kan ikke skjære i
  seg selv, i en annen kutter eller i en gren, og et lag som kuttes kan ikke
  selv bli kutter.
- Lagres i skjelettet, og dermed i angre/gjør om og i .cfproj.

Rettet underveis: skarpe kuttkanter ga hull i meshen (kanter delt av fire
trekanter) der to flater møtes i en tynn kile. Overflateuttrekket løser nå
opp tvetydige sideflater i rutenettet før meshen bygges. Det gjelder alle
kropper, men endrer bare ruter som ellers ville gitt en åpen mesh.

Begrensninger nå:

- Mesh-deler (horn, klør …) i målet kuttes ikke; de legges på etter kuttet.
- Uttrekk med prikkene på målet spilles av etter kuttet. Uttrekk på selve
  kutteren påvirker ikke kuttet.
- En negativ surface-form (punkt 8) finnes ikke ennå.

Testet: `cargo test` (79), `npm test` (22), `npx playwright test` (61 bestått,
35 hoppet over som før; ny spec `negative-shape.spec.ts` på desktop og mobil).
Kjernetestene måler volumet av kula minus linsen der kulene overlapper (innen
3 %), at ingen punkter ligger inne i kutteren, at speilet kutt skjærer på
begge sider, og at meshen er lukket. Ingen fysisk utskrift er gjort.

## Punkt 5: målinger av knekkfortettingen fra PR #10

Kantlengde 2 mm, knekkvinkel 25°. Avvik er |feltverdien| i meshpunktene, som
er avstanden til overflaten bare der feltet er en ekte avstand (ikke i
avrundede ender med profil).

| Form | Nivå | Trekanter | Maks avvik | Snitt avvik |
|------|------|-----------|------------|-------------|
| Stav 60 mm, rektangelprofil | 0 | 2 528 | 3,02 mm | 0,098 mm |
| | 1 | 3 210 | 3,77 mm | 0,030 mm |
| | 2 | 4 482 | 0,64 mm | 0,013 mm |
| | 3 | 7 132 | 0,58 mm | 0,008 mm |
| Kule r = 20 mm | 0 | 3 732 | 0,07 mm | 0,045 mm |
| | 3 | 3 756 | 0,07 mm | 0,045 mm |

Rolige flater (kula) får nesten ingen nye trekanter, og veksten er begrenset
av antall nivåer (maks 3) og en minste kantlengde per nivå. Funn: på nivå 1
øker det største avviket for staven før det faller på nivå 2. Dette bør
undersøkes før punktet regnes som ferdig.

## Åpne spørsmål til Sivert

1. **Arbeidsplan:** Plasseres planet gjennom startpunktet til den nye
   strukturen, og gjelder låsen hele laget (alle grener)? Forslag: ja til
   begge, med planet lagret på laget.
2. **Fyll:** Skal første versjon bare fylle lukkede baner i arbeidsplanet, med
   en fylltykkelse brukeren stiller (ingen standardverdi er bestemt)?
3. **Lokal speiling:** Hvilken akse/side speiles om langs splinen? Forslag:
   planet utspent av tangenten og rammens V, slik at venstre og høyre side av
   splinen speiles, og speilet følger bøyingen.
4. **Meshforming → kontur (punkt 7):** Når en kontur er koblet til en master,
   skal meshforming gjøre den lokale konturen individuell, eller spørre?

## Neste trinn

1. Ringgrep: skaler konturringen direkte i arbeidsrommet (punkt 6), med mm-mål
   under drag og ett angresteg.
2. Arbeidsplan med lås (punkt 1), når spørsmål 1 er avklart.
3. Fyll av lukket bane i arbeidsplanet (punkt 2).
4. Surface Brush (punkt 8), i egne trinn: felt for malt område på underlaget,
   tykkelse og offset, deretter flytting langs overflaten.
