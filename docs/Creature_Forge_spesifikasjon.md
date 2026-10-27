# Creature Forge

## Produktdefinisjon og teknisk funksjonsspesifikasjon

Versjon 1.1 · 27. oktober 2026 · Utarbeidet for Sivert Hovde

Creature Forge skal være et fullverdig modelleringsprogram i nettleseren for å lage egne artikulerte figurer for 3D-print. Brukeren former kroppsdeler direkte, bygger detaljer som overflatefølgende lag og forbinder delene med redigerbare ledd. Under dette skal en geometrimotor håndtere triangulering, deformasjon, sammenføyninger, utskjæringer og kontroll av produksjonsgeometrien.

Dette dokumentet definerer produktet før implementasjon. Det beskriver både brukerens handlinger og geometrien som må ligge bak. Prosjektet starter fra bunnen av i et eget repo. ShapeForge, Godot og eksisterende Designporten-kode er ikke forutsetninger for løsningen.

Det anbefalte tekniske fundamentet er en dokumentmodell med varige formoperasjoner, redigerbare senterlinjer, overflatefester og parametriske ledd. Mesh og avstandsfelt brukes til forskjellige oppgaver. Den viste trianguleringen er en beregnet representasjon og skal ikke være eneste sted modellens mening er lagret.

### Status og lesemåte

**Krav K** er avklart med Sivert i denne samtalen. **Bakgrunn B** er hentet fra overleveringspakken fra 23. oktober og videreføres som tidligere produktretning der den ikke motsies av nyere avklaringer. **Forslag F** er presise anbefalinger i denne spesifikasjonen, ikke brukerbeslutninger. **Verifikasjon V** betyr at oppførselen eller ytelsen må dokumenteres ved implementasjon og tester. Kapittel 22 samler åpne beslutninger.

Krav beskriver tiltenkt ferdig program. Dokumentet hevder ikke at funksjonene er implementert. Forskningsresultater og biblioteker dokumenterer byggesteiner; de beviser ikke at Creature Forge allerede har en fungerende helhet.

## 1 Formål og produktgrenser

**K.** Hovedplattformen er nettleser på PC og Mac, med mobilstøtte. Programmet skal ha reelle redigeringsverktøy og en produksjonsrettet geometrikjerne. Det skal ikke bygges som en visuell mockup som senere må erstattes.

Brukeren skal kunne starte med en kule, forme en kropp, tegne opphøyde eller nedsenkede detaljer og bygge en skapning av flere kroppsdeler. Det samme arbeidsprinsippet skal fungere på en kropp, et hode, en hale og detaljer som ører eller horn. Geometrien skal forbli redigerbar når nye lag og koblinger kommer til.

**B.** Figurenes hovedformål er 3D-print, særlig bevegelige figurer med løse, dinglende ledd. En figur på omtrent 100 mm er et relevant testeksempel, ikke en låst produktstørrelse. Ledd som printes ferdig sammen, er et sentralt mål. Fysiske utskriftstester er nødvendige før en leddprofil kan betegnes som praktisk utprøvd.

**F.** Første produktomfang skal dekke forming, lag, overflateplassering, hengseleditor, sammenstilling, prosjektlagring og eksportkontroll. Kontoer, markedsplass, betaling, generativ AI, samarbeid i sanntid og integrasjon i Designporten inngår ikke automatisk. De er ikke nødvendige for den avklarte modelleringsflyten.

## 2 Begreper og tre forskjellige hierarkier

En **kroppsdel** er en fysisk del av figuren, for eksempel et hode eller en arm. Den kan bestå av mange redigerbare former som til slutt inngår i samme utskriftsvolum. En **grunnform** er kroppens første geometriske form. En **sekundærform** er en form som bygges på en annen overflate. En **Feature** er en detalj med en gjenkjennelig rolle, eksempelvis et øye, horn eller munninnsnitt.

Et **lag** er en ordnet formoperasjon. En **gruppe** beskriver eierskap og felles håndtering. Et **overflatefeste** beskriver hva en form følger geometrisk. Et **koblingspunkt** er et plasserbart feste for et ledd. Et **ledd** beskriver forbindelsen og tillatt bevegelse mellom kroppsdeler.

**K.** Sekundærformer kan ha egne underformer, og disse kan igjen ha underformer. Former som dras langs et underlag, skal følge dets overflate. Sekundærformens egen geometri skal endres etter grunnformens lokale fasong og krumming. Overflatefølging betyr derfor geometrisk deformasjon, ikke bare endring av posisjon og orientering.

**F.** Motoren skal skille mellom tre relasjoner:

- Gruppehierarkiet bestemmer hvilke former som håndteres sammen.
- Overflateavhengigheten bestemmer hvilken tidligere beregnet overflate en detalj bygger på.
- Sammenstillingshierarkiet og leddforbindelsene bestemmer hvordan separate kroppsdeler følger hverandre.

Et øye kan være i gruppen «Hode», følge både hodet og en kinnform som underlag, og samtidig være del av samme fysiske utskriftsvolum som hodet. Gruppen alene kan ikke beskrive dette. En arm festet med et ledd skal derimot være et eget volum og må ikke smeltes sammen med kroppen.

## 3 Arbeidsflyten gjennom en komplett figur

**K.** Brukeren starter med en grunnform, former denne, legger til lag, lager koblingspunkter og bygger videre med ferdige kroppsdeler eller nye kuler. Rekkefølgen er en arbeidsflyt, ikke en lås: ferdige former skal kunne redigeres igjen.

**K.** Brukeren skal se et tomt arbeidsrom når appen starter; ingen form lastes automatisk (avklart med Sivert 27. oktober 2026).

**F.** En sammenhengende økt skal fungere slik:

1. Appen og et nytt prosjekt starter med et tomt arbeidsrom. Velg dimensjoner i millimeter og legg til en kule med et tydelig valg, for eksempel «Legg til kule».
2. Dra i et kontrollpunkt eller marker et meshområde og trekk det ut.
3. Bøy uttrekkingen med Alt eller Alt + Shift. Slipp musen og rediger videre med den varige senterlinjen.
4. Legg til et lag. Tegn et lukket omriss på den nåværende overflaten.
5. Trekk den tegnede formen ut til ønsket tykkelse. Juster avrunding, offset og eventuell sammensmelting.
6. Form detaljer videre, eller legg underformer i detaljens gruppe. Flytt dem langs overflaten ved behov.
7. Plasser et koblingspunkt. Velg hengsletype og retning, og juster mekaniske dimensjoner og klaringer.
8. Legg til en ferdig kroppsdel eller form en ny. Koble delene med et kompatibelt feste.
9. Undersøk leddets bevegelse og modellens produksjonsgeometri. Rett feil i originaloperasjonene.
10. Lagre redigerbart prosjekt og eksporter et kontrollert øyeblikksbilde til utskrift.

## 4 Grunnform og direkte grep i meshen

### Kontrollpunkter

**K.** Når pekeren hviler over et kontrollpunkt, vises en pil som angir uttrekksretningen. Avstanden mellom drapunkter kan angis i sidepanelet. Flere punkter blir tilgjengelige når overflaten vokser. Brukeren skal også kunne gjøre formen mindre.

**F.** Punktene fordeles over overflaten med en avstand målt langs denne, så langt det er praktisk mulig. Punktavstand i millimeter er en håndteringsinnstilling. Den er forskjellig fra geometrisk oppløsning. En detaljert mesh kan derfor ha få synlige grep. Punktvisningen kan tilpasses zoom, men skal ikke endre den lagrede geometrien.

Et aktivt grep får varig identitet mens det brukes. Nye grep kan fordeles etter endring av formen uten at det aktive grepet hopper. Normal og pil beregnes fra et lokalt overflateområde, slik at små endringer i trianguleringen ikke gir flimrende retning.

### Markering av meshområder

**K.** Brukeren kan markere deler av meshen og trekke området ut samlet. De samme bøyeprinsippene skal være tilgjengelige.

**F.** Markering skjer med pensel eller frihåndsomriss. Bare synlig overflate velges som standard; valg gjennom modellen er en eksplisitt modus. Markeringen lagres som et område med påvirkningsvekter, ikke bare som en liste over aktuelle trekantnumre. Rundt det valgte området ligger en overgangssone som gir myk tilkobling til resten av formen.

For et punktgrep brukes en justerbar påvirkningsradius. For et valgt område brukes det markerte fotavtrykket. Ved sterkt sprikende normaler skal appen vise og la brukeren korrigere retningen før uttrekking. To atskilte områder skal ikke stille bli tolket som én sammenhengende utvekst.

### Uttrekking og innpressing

**F.** Uttrekking bygger en forlengelse av den valgte formen med en lagret rot, påvirkningssone og senterlinje. Innpressing flytter overflaten innover, med løpende kontroll av kryssing og tilgjengelig veggtykkelse. Geometrien skal endres; en skalering eller flytting av hele objektet er en annen operasjon.

En utvekst skal ikke få en skjult kobling til skjermens dybdeakse. Ved vanlig uttrekking låses pilretningen fra starten av draget. Visningen viser aktiv retning og dimensjon. Hvordan museavstand oversettes til millimeter, er en justerbar følsomhet og skal gi samme geometriske resultat uavhengig av skjermoppløsning.

## 5 Alt og den låste tråden

### Den avklarte oppførselen

**K.** A er startpunktet før draget. Først trekkes grepet B ut fra A, rett ut i forhold til overflaten. Når Alt + Shift holdes inne, låses den nåværende lengden på tråden mellom A og B. B kan deretter beveges rundt A. Dette styrer bøyningen og retningen på den uttrukne meshen.

Geometrisk uttrykkes begrensningen slik: B = A + L · d. Her er L den låste avstanden og d en enhetsvektor. A står fast gjennom dette grepet. Avstanden |B − A| skal være L innenfor numerisk toleranse. Det er endepunktets bevegelsesflate som er kuleformet. Verktøyet krever ikke at hele utveksten blir en matematisk halvkule.

**K.** Med Alt alene beveges grepet i XY relativt til utgangspunktet. Dette er en annen måte å bøye eller rette den uttrukne formen på.

**F.** XY defineres som et lokalt plan vinkelrett på den opprinnelige uttrekksretningen. Planet vises ved behov og fryses ved inngang til modusen. Denne koordinatdefinisjonen må bekreftes i betjeningstesten; globalt XY og skjermplanet er mulige alternativer som ikke skal blandes stille.

### Oversettelse fra mus til tredimensjonal retning

**F.** Bruk et relativt virtuelt kulegrep. Ved inngang til Alt + Shift lagres A, L, retningen d og kameraets orientering. Musebevegelse oversettes til små rotasjoner omkring kameraets høyre- og oppretning. Rotasjonene oppdaterer d, som normaliseres før B beregnes på nytt. Dette gir tilgang til retninger også utenfor kuleflatens synlige silhuett og unngår hopp når en musestråle ikke treffer kulen.

Kameraet skal ikke samtidig rotere under samme drag. Bytte mellom vanlig dra, Alt og Alt + Shift rebases fra grepets nåværende posisjon, slik at posisjonen ikke hopper. Når Alt + Shift aktiveres på nytt, låses den lengden grepet har da. Ved null eller nesten null lengde må brukeren først trekke ut en målbar lengde. Escape avbryter hele draget og gjenoppretter utgangspunktet.

### Låst tråd og buelengde er forskjellige størrelser

**V.** L er den rette avstanden A–B. Den er ikke automatisk lik lengden langs en bøyd senterlinje. Det er heller ikke nok informasjon i A og B til å bestemme én unik kurve, et tverrsnitt eller vridning omkring lengdeaksen.

**F.** Motoren bruker i tillegg rotens tangent, eksisterende kurve, en eksplisitt regel for krumning og transport av tverrsnitt. Grunnmodusen låser trådlengden, slik brukeren har definert. En eventuell separat funksjon for å bevare materiallengde må være et eget valg. Programmet skal ikke love begge egenskaper samtidig for vilkårlige grep.

## 6 Senterlinjen som varig redigeringsverktøy

**K.** Når musen slippes, beholdes en redigerbar senterlinje gjennom den uttrukne formen. Meshen følger senere endringer av linjen og får oppdatert triangulering.

**F.** Hver uttrekking lagres som en egen formoperasjon med rotfeste, kurve, tverrsnittsbeskrivelse og påvirkningsområde. Den innledende kurven beregnes som en glatt romkurve. En kubisk Hermite-kurve eller en kurve med flere slike segmenter er aktuell fordi endepunkter og tangenter kan styres separat. Konkret kurveregel velges ved tester av ønsket oppførsel, ikke ut fra at to punkter alene skulle være tilstrekkelig.

Roten skal gli mykt inn i kroppen. Etter at formen er opprettet, kan brukeren velge linjen, flytte kontrollgrep og endre bøyningen. Innsetting av ekstra kurvegrep er et forslag for å gi flere buer på samme utvekst. Sidepanelet viser lengder, påvirkningsradius og tverrsnitt der disse er relevante.

En senterlinje styrer bare sin egen formoperasjon. Den skal ikke automatisk skape et fysisk ledd. Bøyning under modellering og artikulasjon mellom separate deler er forskjellige operasjoner.

### Hvordan meshen følger kurven

**F.** Tverrsnitt og lokale detaljer bindes til en referansetilstand langs kurven. Ved endring av kurven flyttes de ved hjelp av lokale koordinatrammer, med bevart lokal form som mål. Et rotområde glatter overgangen til kroppen. Restavvik fra senere fri forming lagres i den lokale referansen, slik at små detaljer kan følge med.

Ved generering av nye meshpunkter overføres tilhørende kurveposisjon og lokale koordinater. Det skal være mulig å evaluere formen uten de opprinnelige trekantene. Senere operasjoner har egne avhengigheter; hvis en tidligere endring gjør dem ugyldige, skal de flagges og bevares for reparasjon.

**Research.** Rotasjonsminimerende koordinatrammer er en etablert metode for å føre tverrsnitt langs romkurver med liten uønsket vridning. Wang med flere beskriver en stabil beregningsmetode. Dette støtter valg av rammetransport, men bestemmer ikke den ønskede kurveformen i Creature Forge. [S2]

ARAP og andre formbevarende deformasjoner er aktuelle som lokal korreksjon, særlig ved uregelmessige markeringer. De må testes for endringer i volum, kollaps og kjøretid; de gir ikke automatisk kollisjonsfrihet. libigl dokumenterer flere relevante deformasjonsmetoder. [S3]

## 7 Dynamisk remeshing under alle formendringer

**K.** Meshen skal oppdateres når formen trekkes ut, bøyes, presses inn eller gjøres mindre. Nye trekanter legges til der geometrien trenger dem. Fortetting under krymping skal også håndteres.

**F.** Motoren har en lokal arbeidsmesh med nabolagsinformasjon. Etter små deformasjonstrinn deler den for lange kanter, slår sammen for korte kanter, bytter egnede kantforbindelser og fordeler punkter bedre. Dette skjer i det påvirkede området og en rand rundt. Randbetingelser, skarpe detaljer, materialgrenser og mekaniske fester beskyttes etter egne regler.

Som innledende algoritmeparameter kan deling prøves ved kantlengde over 4h/3 og sammenslåing under 4h/5, der h er ønsket lokal kantlengde. Dette er foreslåtte terskler som skal testes og kan erstattes; de er ikke brukerkrav eller kvalitetsgarantier.

Tettheten tilpasses både krumning, små detaljer og geometrisk feil. Store, rolige flater trenger færre trekanter enn små øyelokk og trange klaringer. Løsningen skal kunne øke detaljgraden lokalt uten at hele figuren får samme tetthet.

**Research.** CGAL beskriver lokale remeshingoperasjoner med deling, kollaps, kantbytte og flytting av punkter. Dette er en algoritmisk referanse. Det er ikke et krav om å bruke CGAL, og en remesher som projiserer tilbake til et input, må bruke den oppdaterte formen som mål, ikke trekke skulptureringen tilbake til den gamle overflaten. [S1]

### Kvalitet under drag og ved slipp

**F.** Remeshing kjører gjennom draget i tidsavgrensede steg. Etter slipp fullføres restarbeid og kontroll før operasjonen regnes som ferdig beregnet. En lettere forhåndsvisning må vise samme formintensjon og skal ikke gi et vesentlig annet enderesultat uten synlig beskjed.

Ugyldig topologi, inverterte elementer eller numeriske feil skal ikke skrives over siste gyldige tilstand. Motoren prøver mindre steg eller stopper operasjonen med markert feilområde. Hele modellen skal ikke glattes eller slås sammen automatisk for å skjule et lokalt problem.

## 8 Penneverktøyet og overflatefølgende lag

**K.** Når grunnformen er klar, kan brukeren tegne en ny form utenpå med penneverktøyet. Den tegnede formen kan deretter trekkes ut til ønsket tykkelse og få avrundede former. Et strøk kan gå over både grunnformen og tidligere vektorblobs. Hele formen skal følge underlaget.

**F.** Penneverktøyet lager et lukket, redigerbart omriss med kurvepunkter og tangenthåndtak. Brukeren kan tegne frihånd eller bygge konturen punktvis. Frihånd forenkles innenfor en oppgitt avvikstoleranse; originalprøvene kan beholdes for senere justering. Konturen er en kurve på overflaten, ikke bare skjermkoordinater.

Under tegning treffes den synlige, tillatte underlagsflaten. Konturen samles opp med nok prøver til å følge krumningen. Ved rotasjon av kamera under en pause i tegningen kan konturen fortsettes på baksiden. Lukking skal være eksplisitt eller skje med en synlig lukkeindikasjon.

Når konturen er lukket, beregnes også innsiden mot underlaget. Appen må håndtere innvendige høydeforskjeller selv om ytterkanten ligger på et rolig område. En flat triangulert skive mellom kantpunktene oppfyller ikke kravet.

**F.** Første utgave av konturmodellen tillater lukkede, ikke selvkryssende omriss. Selvkryssing markeres med mulighet for å rette den. Det skal ikke produseres vilkårlig fyll uten forklaring. Flere omriss, hull og eksakt fyllregel må eventuelt utvides som en dokumentert funksjon.

### Underlaget ved overlapp

**F.** Et nytt lag evalueres mot en versjon av den samlede overflaten under laget i sin kroppsdel. Dette omfatter grunnformen og tidligere relevante lag. Laget selv, senere lag og separate kroppsdeler er utelatt. Lagrekkefølgen er dermed også en beregningsrekkefølge.

Ved glatte overganger er underlagets normal entydig. På skarpe sømmer eller der to overflater møtes, finnes ikke alltid én glatt normal. Motoren trenger en eksplisitt sømregel: enten avrundet underlag innenfor toleranse eller en markert grense som krever justering. Skjult utjevning som endrer mekaniske mål, er ikke tillatt.

Hvis et underlag endres, beregnes avhengige lag på nytt. **K.** Sekundærformens geometri skal tilpasses den endrede fasongen og krummingen i underlaget. Flytting av et lag i rekkefølgen kan endre hva det ligger på. Dette skal forhåndsvises og kunne angres.

## 9 Tykkelse offset avrunding og sammensmelting

Disse egenskapene har forskjellige betydninger og skal ha separate parametere.

### Tykkelse

**K.** Uttrekkingen av en tegnet form skal bygge tykkelse i forhold til underlagets lokale normaler. Alle punktene skal ikke flyttes i én felles retning.

**F.** På et glatt område beskrives den ønskede topplaten lokalt ved p + (o + t) · n, der p er underlagspunktet, n er lokalenormalen, t er tykkelsen og o er offset. Den foreslåtte bunnflaten ligger ved p + o · n. Sideflater og kantprofil lukker volumet. Uttrykket definerer intensjonen lokalt; det er ikke tilstrekkelig som komplett robust algoritme.

**Research.** Normalforskyvning kan gi selvkryssing og varierende faktisk tykkelse i krevende områder. Blender dokumenterer slike begrensninger også i en etablert Solidify-funksjon. Creature Forge må derfor beregne og kontrollere sluttgeometrien, ikke anta at en normalforskyvning alltid lykkes. [S4]

### Offset

**K.** Den tegnede formen skal ha en etterfølgende offsetjustering i forhold til grunnformen. Formen skal beholde overflatefølgingen.

**F.** Offset flytter formens plassering inn eller ut fra det evaluerte underlaget, uten å være samme parameter som tykkelse. Ved kryssing over tidligere lag følger den det samlede underlaget. En eventuell ekstra offset målt direkte fra den opprinnelige grunnformen må være en separat referansemodus; den kan ikke samtidig behandles som offset fra toppen av en annen blob.

En positiv offset kan skape en fysisk løs del. Programmet skal vise dette som resultat, ikke automatisk forlenge formen ned til kroppen. For et additivt lag som skal inngå i samme printdel, må brukeren justere offset, tykkelse eller sammenføyning slik at det finnes en reell forbindelse.

### Avrunding og sammenføyning

**K.** De tegnede formene skal kunne få avrundede former. **B.** Additive Features kan smeltes inn med individuell blending; negative Features kan skjære bort geometri.

**F.** Avrunding styrer profil og kantovergang i selve formen. Blending styrer sammensmelting med andre former. De skal ikke være én felles glidebryter. Et bredt, lavt felt kan få en rund profil, mens et horn kan få en smal base og tydelig topp. Eksakt profilfamilie er et implementasjonsvalg som skal vurderes visuelt og geometrisk.

Ved store tykkelser eller offseter må motoren oppdage lokale kryssinger, sammenstøt med annen geometri og tap av små detaljer. Den viser ugyldige områder og mulige endringer. Den skal ikke stille klippe tykkelsen ned mens sidepanelet fortsatt viser den opprinnelige verdien.

## 10 Videre forming og underformer

**K.** En ferdig tegnet form kan redigeres videre ved å dra ut pigger, øyne og andre detaljer. En sekundærform kan ha egne former i gruppen sin. Alle former skal følge overflaten de dras rundt på.

**F.** Brukeren kan velge enten selve formen, konturen, en lokal meshmarkering eller en senterlinje. Valget avgjør redigeringen. Uttrekking av en pigg fra en sekundærform oppretter en ny varig formoperasjon. Flytting av hele sekundærformen endrer overflatefestet, ikke bare verdenskoordinatene til den ferdige meshen.

**K.** Når en sekundærform dras utenpå grunnformen, skal selve sekundærformens geometri endres i forhold til hvordan grunnformen er formet. Den skal bøyes og tilpasses det lokale underlaget. Det er ikke tilstrekkelig å beholde en stiv mesh og bare flytte eller rotere den slik at festepunktet følger overflaten. Den samme tilpasningen skal skje når grunnformen senere endres.

Underformer følger med ved gruppeflytting. **F.** Sekundærformen evalueres fra sin redigerbare konstruksjon mot det aktuelle underlaget. Området den dekker, følger underlagets fasong, og tykkelse, offset og uttrukne detaljer bygges videre fra denne tilpassede geometrien. Tilpasningen gjelder hele formen over området, ikke bare kontur eller festepunkt. De utstikkende delene skal fortsatt ha volum; alle toppunkter skal ikke projiseres flatt ned på grunnformen.

Et eksempel er en tegnet flekk som flyttes fra et flatt område til siden av en rund kropp. På det flate området er flekken flatere. På den runde siden bøyes den rundt kroppen. Flyttes den videre til et smalere område, beregnes geometrien på nytt etter dette underlaget. Endring av kroppen under flekken skal også oppdatere flekkens geometri.

Hengsler er et viktig spesialtilfelle. De skal feste seg og orienteres mot overflaten, men mekanisk geometri skal være dimensjonsstabil som standard. Hele hengslet skal ikke bøyes for å passe kroppen. I stedet kan festebasen tilpasses underlaget. Dette er en foreslått teknisk regel for å bevare klaringer.

### Hvordan flytting over overflaten beregnes

**F.** Flytting følger en sammenhengende bane på det valgte underlaget. Feste, lokal normal og tangentretning oppdateres langs banen. En enkelt nærmeste-punkt-beregning er ikke tilstrekkelig ved tynne kroppsdeler eller tettliggende flater, fordi detaljen da kan hoppe til baksiden.

Retningen i overflaten transporteres kontinuerlig. Brukerens egen rotasjon lagres som et tillegg. Ved store formforskjeller kan et fotavtrykk strekke eller komprimeres; dette vises og måles. Nøyaktig uendret størrelse og perfekt anlegg på vilkårlig krum overflate kan ikke garanteres samtidig.

**Research.** Geometry Central tilbyr geodetisk sporing og transport av tangentretninger. Dette er relevante metoder for overflateflytting, men de løser ikke alene festeoverføring når topologien endres. [S5][S6]

## 11 Stabile fester gjennom remeshing

**F.** Hver kurve, detalj og kobling får en referanse til et logisk underlag og en referansetilstand. Trekantindekser og barysentriske koordinater kan brukes som hurtigdata, men skal ikke være eneste identitet.

For deformasjoner som har kjent avbildning, transporteres festene med den samme avbildningen. Når remeshing erstatter punkter og flater, overføres de lokale feltene og festene til den nye meshen. Ved endringer som skaper eller fjerner overflater, brukes kontrollert reprojeksjon med krav til riktig vert, forventet side, normalretning og maksimal flytteavstand.

Hvis flere mulige overflater konkurrerer, skal systemet ikke velge vilkårlig. Formen får status «Feste må repareres», beholder siste gyldige tilstand og lar brukeren velge riktig underlag. Hvis et lag som bærer andre former slettes, må brukeren velge om underformene skal slettes, beholdes frakoblet eller festes på nytt. Ingen av disse handlingene skal skje skjult.

Dette er et område som krever egen utvikling og verifikasjon. Det finnes ingen dokumentert universell bibliotekfunksjon som alene garanterer alle disse egenskapene for vilkårlig fri forming og lagdeling.

## 12 Speiling og symmetri

**K.** Mirror kan aktiveres og deaktiveres. Det gjelder formarbeid og plassering av hengsler med tilhørende negative former.

**F.** Symmetriplanet tilhører kroppsdelen eller sammenstillingen og vises når speiling er aktiv. Brukeren velger om speiling gjelder et nytt grep, en form eller en kobling. Et speilet par har en eksplisitt relasjon, ikke bare to tilfeldig like kopier.

Ved deaktivering skal eksisterende geometri beholdes. Forslaget er at koblingen i et valgt par kan brytes slik at begge blir selvstendig redigerbare. Å slå speiling på igjen skal ikke automatisk overskrive asymmetriske endringer; brukeren velger kildeside og ser resultatet før gjennomføring.

Midtlinjen skal ikke få doble, overlappende uttrekk eller negative volumer. Refleksjon må også korrigere orientering av trekanter og lokale leddakser. Speiling av et hengsel skal gi et mekanisk korrekt speilbilde, ikke negative dimensjoner som endrer klaringene.

**B.** Ferdige øye-Features kan plasseres som par med bevart symmetri. Den globale speilingsinnstillingen og et slikt pars forhold må være synlige, slik at brukeren forstår hvilken side som påvirkes.

## 13 Koblingspunkter og sammenstilling

**K.** Når en kroppsdel er formet, kan koblingspunkter plasseres på den. I sidepanelet velges retning innover eller utover, lengde, vinkler, tykkelse, leddklaring og klaring mellom kroppsdelene. Neste del kan være ferdiglaget eller starte fra en ny kule.

**F.** Koblingspunktet har et overflatefeste og en lokal koordinatramme. «Innover» og «utover» defineres i forhold til vertens normal, mens hengslets mekaniske akse vises separat. Vinkler angis relativt til denne lokale rammen; verdensrotasjon er en annen visning.

En ny kroppsdel får et kompatibelt motfeste. Ved sammenkobling viser appen hvilke fester som kobles, leddets retning og avstand mellom delene. Forhåndsdefinerte fester kan finnes i ferdige deler, men er ikke lenger eneste lovlige koblingsmåte. Dette erstatter den eldre pakkens krav om bare forhåndsdefinerte snappunkter.

Flytting av en overordnet del flytter tilknyttede barn uten å deformere dem. Posing gjennom et ledd endrer stilling innenfor leddets frihetsgrader. Redigering av selve kroppen endrer vertens geometri og krever ny beregning av festet. Brukeren skal kunne skille mellom disse tre handlingene.

**F.** Første sammenstillingsmodell bruker en rot og et tre av deler. Lukkede mekaniske kjeder krever en egen løser for begrensninger og inngår ikke automatisk. Løse kjettingledd trenger dessuten flere frihetsgrader enn et vanlig enakset hengsel.

## 14 Egen editor for hengsletyper

**K.** Hengsler skal ha en egen editor der brukeren kan lage og redigere egne typer. En type kan ha negativ geometri som automatisk graver bort mesh for å skape bevegelighet.

**F.** Hengseleditoren åpnes som en egen arbeidsmodus med isolert 3D-visning, parametere og bevegelsesprøve. Den bruker to representative kroppsdeler eller de faktiske vertsdelene. Brukeren kan vise og skjule positiv geometri, negativ geometri, festebaser, akser og klaringsområder.

En hengsletype består av en definisjon med stabile parameter-ID-er og versjon. En plassert hengselinstans lagrer typeversjon, egne parameterverdier og koblede kroppsdeler. Endring av typen skal ikke stille endre alle ferdige figurer. Oppgradering av instanser skal kunne forhåndsvises og angres.

### Egenskaper i en hengsledefinisjon

| Egenskap | Betydning |
| --- | --- |
| Feste A og B | Lokale tilkoblingsrammer og kompatibilitet |
| Positive former | Geometri som inngår i én av de bevegelige delene |
| Negative former | Volumer som fjernes fra uttrykkelig valgte vertsvolumer |
| Frihetsgrader | Tillatt rotasjon og eventuell forskyvning |
| Grenser | Bevegelsesgrenser eller begrensning bestemt av kontakt |
| Mekaniske mål | Lengde, tykkelse, diameter og orientering etter type |
| Leddklaring | Avstand mellom mekaniske motflater |
| Kroppsklaring | Avstand mellom tilstøtende kroppsdeler |
| Beskyttede soner | Områder der utskjæring ikke skal fjerne nødvendig feste |
| Profilreferanse | Printer, materiale og utskriftsparametere brukt ved testing |

Dette er foreslått datastruktur. Brukeren skal kunne bygge egne typer av redigerbare former og parametere, men trenger ikke programmere geometri for å plassere en eksisterende type.

### Negative former og utskjæringsrekkefølge

**F.** Negative former følger hengslets plassering, orientering og relevante mål. Hvert negativt volum har en liste over målvolumer det får skjære i. Det skal ikke som standard kutte den andre hengseldelen, nærliggende lemmer eller hele figuren.

Når hengslet flyttes eller endres, beregnes utskjæringen fra vertens tidligere ukuttede tilstand. Det gamle hullet skal dermed forsvinne. Gjentatt destruktiv utskjæring i allerede skåret mesh er ikke en egnet dokumentmodell.

Hvis kutteren bryter gjennom huden, svekker festet eller treffer andre deler, vises dette som et konkret geometrisk resultat med feilmarkering. Automatisk utskjæring skal ikke automatisk få status som kontrollert bevegelighet.

## 15 Hengslets bevegelsesrom og klaringskontroll

**F.** Brukeren kan legge til en egen negativ form manuelt. I tillegg anbefales en funksjon som beregner et bevegelsesrom fra den bevegelige delens form og tillatte stillinger. Dette er et utvidet teknisk forslag, ikke en erstatning for egne kuttere.

For en bevegelig form M, bevegelse T(q) og tillatt stillingsområde Q er idealet volumunionen av T(q)M for alle q i Q. Et klaringstillegg må deretter inngå i beregningen. Roterer delen rundt én akse, kan q være én vinkel. For et løst ledd er området flerdimensjonalt og betydelig mer krevende.

**Research.** Et swept volume beskriver volumet et objekt opptar gjennom en bevegelse. libigl har en metode som bruker diskrete tidsprøver. Det betyr at et beregnet volum fra denne metoden ikke uten videre er et eksakt omslag for alle mellomstillinger. [S7]

**F.** Bruk adaptiv prøvetaking med konservativ feilmargin eller andre dokumenterte omslagsmetoder. Kontroller med uavhengige avstands- og kollisjonstester. Kontinuerlig kollisjonskontroll forutsetter en definert bevegelse og en løser som støtter den aktuelle geometrien. En animasjon med noen få teststillinger er ikke bevis for fri bevegelse.

**Research.** FCL dokumenterer avstand, toleranse og kontinuerlig kollisjonskontroll. Det gjør biblioteket relevant som referanse og kandidat for kontroll, men tilgjengelighet og ytelse i vår nettleserarkitektur er ikke verifisert. [S8]

For sammenflettede løkker må kontrollen også vurdere at løkkene er lukket og at delene ikke kan løsne gjennom sitt tillatte bevegelsesrom. For andre ledd kreves egne fangstbetingelser. Kollisjonsfrihet i noen stillinger er ikke tilstrekkelig til å fastslå at et ledd holder sammen.

### Tre forskjellige kuppelbegreper

Kuppelgrepet i kapittel 5 er en musebegrensning. Den negative kuppelen fra den eldre overleveringspakken er en mulig kutterform. Et beregnet bevegelsesvolum kan ha en helt annen form. Disse funksjonene skal ha forskjellige navn og separate parametere i kode og grensesnitt.

## 16 Geometrimotorens representasjoner

**F.** Den anbefalte motoren kombinerer tre representasjoner med tydelig eierskap:

- Dokumentet lagrer formintensjon, operasjoner, relasjoner og referansetilstander.
- Arbeidsmeshen brukes til markering, direkte forming, overflateberegning og visning.
- Volumrepresentasjon brukes der sammenføyning, negative former eller offset trenger volumoperasjoner.

For organiske overganger er implisitte felt og avstandsfelt aktuelle. OpenVDB dokumenterer sparsom volumlagring og boolske operasjoner. Dette er et metodegrunnlag, ikke et valg om å laste hele OpenVDB inn i mobilnettleseren. Binærstørrelse, avhengigheter og minnebruk må vurderes særskilt. [S9]

Et signert avstandsfelt har negativ verdi inne, positiv utenfor og null på overflaten etter valgt konvensjon. Union kan beskrives med min og differanse med max(fA, −fB). Slike kombinasjoner gir en implisitt form, men det kombinerte feltet er ikke nødvendigvis en korrekt euklidsk avstand overalt. Glatte blandinger og ikke-uniform skalering gjør avstandsmåling enda mer krevende. Motoren må derfor ikke bruke vilkårlige feltverdier som ferdig klaringsmål.

Mekaniske grenseflater skal ha dokumentert feilbudsjett. Finere oppløsning skal kunne brukes lokalt. Ingen global remeshing eller glatt union får lukke spalten mellom bevegelige deler uten at dette oppdages.

### Boolske operasjoner og avrunding

**Research.** Manifold tilbyr solide meshoperasjoner og en eksisterende WebAssembly-kobling. CGAL beskriver også robuste meshboolske operasjoner og betydningen av eksakte geometriske beregninger ved gjentatte operasjoner. Ingen av dem leverer hele Creature Forge-arbeidsflyten. [S10][S11]

**F.** Motoren skal skille mellom skarpe boolske operasjoner, organiske blandinger og lokal kantavrunding. En visuelt myk skyggenormal endrer ikke geometrien og oppfyller ikke kravet om fysisk avrunding. For print skal avrundingen finnes i det eksporterte volumet.

## 17 Anbefalt nettleserarkitektur

**F.** En hensiktsmessig start er TypeScript for grensesnitt og applikasjonsflyt, Three.js for visning og en egen geometrikjerne kompilert til WebAssembly. Dette er tekniske forslag som skal bekreftes med en byggbar motorprøve.

Rust er en aktuell kandidat for dokumentmodell, lokale meshdata, algoritmer og tester. C++-biblioteker kan være aktuelle for særskilte operasjoner. Det må måles om flere språk og Wasm-moduler gir forsvarlig kompleksitet og datakopiering. Å velge Rust betyr ikke at nødvendige algoritmer allerede finnes som ferdige Rust-biblioteker.

**Research.** wasm-bindgen støtter kobling mellom Rust og JavaScript og kan generere TypeScript-bindinger. Dette bekrefter en praktisk integrasjonsvei, men er ingen garanti for geometriytelse. [S12]

### Ansvarsdeling

Grensesnittet eier paneler, tastatur, berøring og brukerens verktøyvalg. Et samhandlingslag oversetter dette til kommandoer med millimeterverdier og lokale referanser. Geometrikjernen eier dokumentets geometritilstand. Visningen mottar beregnede meshdata og utvalgsmarkører; den skal ikke være en parallell, konkurrerende kilde til dokumentets sannhet.

Tunge geometrioperasjoner kjører i en Web Worker. Kommunikasjonen bruker versjonerte kommandoer, dokumentrevisjoner og overføring av store buffere der det er egnet. Et gammelt beregningsresultat får ikke overskrive en nyere redigering. Operasjoner må støtte avbrytelse eller trygg forkasting av foreldede resultater.

**Research.** Overførbare ArrayBuffers kan flyttes mellom tråder uten en ekstra kopi, men avsenderen mister eierskapet. Bufferhåndteringen må derfor være eksplisitt. Hvis C++-moduler bruker Emscripten-pthreads, krever det SharedArrayBuffer og passende COOP/COEP-oppsett. Dette skal ikke forveksles med vanlig bruk av én worker, som ikke i seg selv krever delt minne. [S13][S14]

### Grafikk og maskinvare

**F.** WebGPU er en ønsket visnings- og beregningsmulighet, med testet WebGL 2-visning der WebGPU mangler. Kjernefunksjonene skal ikke være avhengige av en GPU-compute-funksjon som mangler en reell alternativ algoritme.

**Research.** Three.js dokumenterer WebGPU-rendering med WebGL 2-reserve. Rendererens reserve betyr ikke at egendefinerte compute-operasjoner automatisk får samme funksjon i WebGL. Nettleseren må testes for faktisk tilgjengelige funksjoner, og WebGPU krever sikker kontekst. [S15][S16]

Serverbasert geometri er ikke en vedtatt forutsetning. Hvis enkelte operasjoner senere krever server, må dette spesifiseres som en synlig produktavhengighet med håndtering av frakobling, ventetid og modelloverføring.

## 18 Brukergrensesnitt og mobilbetjening

**B.** Retningen er et lyst, rolig og profesjonelt grensesnitt med stor 3D-arbeidsflate. Kroppsdeler og modellbibliotek kan ligge nederst, Features i et eget panel og egenskapene til aktivt objekt til høyre. Bare én undermeny skal være åpen av gangen. Funksjonsikoner skal være SVG; modellminiatyrer skal avspeile faktisk geometri.

**F.** De aktive arbeidsmodusene er Form, Tegn, Plasser og flytt, Senterlinje, Koblinger og Kontroller. Brukeren kan gå tilbake til tidligere moduser. Det skal alltid være tydelig hvilket objekt og hvilket underlag som redigeres. Meshlinjer, kontrollpunkter, kurver og negative volumer kan vises ved behov og skal ikke permanent fylle arbeidsflaten.

Sidepanelet skiller mellom kontrollpunktavstand, meshkvalitet, påvirkningsradius, tykkelse, offset, avrunding og blending. For et ledd viser det i stedet relevante mekaniske parametere og målt kontrollstatus. Numeriske felt skal støtte presise verdier og enheter i tillegg til dragging.

### Mus og tastatur

**K.** Alt og Alt + Shift styrer formingen som beskrevet. På Mac tilsvarer Alt normalt Option-tasten. **F.** Samme funksjoner finnes også som synlige moduser, slik at tastaturkombinasjonene ikke er eneste tilgang. Hvis nettleser eller operativsystem fanger en snarvei, skal brukeren fortsatt kunne utføre operasjonen.

Primærdrag på aktivt grep redigerer. Kamerabevegelse får en egen handling. Escape avbryter. Angre skal gjelde hele det ferdige draget, ikke hvert museevent. Fokus i et tallfelt skal ikke samtidig utløse formverktøyets snarveier.

### Mobil og berøring

**K.** Mobil skal støttes. **F.** Synlige valg for «Trekk ut», «Flytt i plan» og «Lås lengde og bøy» erstatter behovet for modifikatortaster. Ett fingerdrag på aktivt grep redigerer; to fingre brukes til kamera. Overgang til kameragest skal ikke tilfeldigvis bekrefte en formendring. Berøringsavbrudd skal håndteres som en kontrollert fullføring eller avbrytelse etter en dokumentert regel.

Ved manglende hover vises pilen når et grep velges. Panelene blir skuffer med større treffområder. De samme dokumentparametrene og operasjonene gjelder på mobil. En lettere visning skal ikke endre modellens lagrede kvalitet eller eksporttoleranse. Store modeller kan kreve at brukeren venter på beregning eller arbeider med færre synlige detaljer; endelig kapasitet må måles.

## 19 Dokumentmodell lagring og angre

**F.** Prosjektet skal lagre nok til å fortsette konstruksjonen. Det trenger stabile ID-er, enhetsangivelse, formatversjon og versjon av geometriberegningene. En samling eksporterte trekanter er ikke et komplett redigerbart prosjekt.

Foreslåtte hovedobjekter er Project, BodyPart, BaseShape, PullFeature, Centerline, SurfacePatch, SurfaceAnchor, FeatureGroup, MirrorRelation, JointDefinition, JointInstance og ValidationReport. Hver formoperasjon refererer til sine innganger og parametere. Sammen med referansegeometri og nødvendige lokale avvik skal dette gjøre gjenberegning mulig.

Beregningsrekkefølgen er en asyklisk avhengighetsgraf. Endring av grunnformen ugyldiggjør de avledede delene som faktisk berøres. Mellomresultater kan caches med versjon og hash. Synlighet alene trenger ikke endre konstruksjonen; en separat funksjon deaktiverer en operasjon i beregningen.

### Angre og gjenoppretting

Hvert drag er én transaksjon. Endring av hengseltype, speilingsforhold og lagrekkefølge skal også være atomiske operasjoner. Angre må gjenopprette parametere, fester og avhengigheter, ikke bare siste meshbilde. For dyre eller topologiendrende operasjoner anbefales sjekkpunkter i tillegg til kommandohistorikk.

Prosjektet får automatisk gjenoppretting i nettleseren og eksplisitt nedlasting av en prosjektfil. Foreslått filending er .cfproj, men dette er ikke et avklart brukerkrav. Et arkiv kan inneholde et manifest, dokumentdata, referansegeometri og valgfrie cachefiler. Formatet må tåle migrering uten stille tap av funksjoner.

**Research.** IndexedDB kan lagre strukturert data og filer asynkront. Nettleserlagring har likevel kvoter og kan slettes etter nettleserens regler. Automatisk lagring i nettleseren må derfor ikke presenteres som en ubetinget sikkerhetskopi. [S17][S18]

Import må kontrollere formatversjon, ugyldige tall, ressursgrenser og manglende referanser før data tas inn i aktivt dokument. Ukjente objekter skal bevares der det er mulig og tydelig markeres. Lagrede cachefiler skal kunne kastes uten at hele konstruksjonen går tapt.

## 20 Produksjonsgeometri og eksport

**B.** Modellen skal kontrolleres for lukkede volum, sammenheng, klaring, veggtykkelse og forhold som påvirker print. **F.** STL og 3MF anbefales som eksportformater. Prosjektfilen er separat og skal fortsatt være kilden til redigering.

Eksport skjer fra en fast dokumentrevisjon. Først beregnes hver kroppsdel med alle aktive former og utskjæringer. Additive detaljer som skal være samme fysiske del, føyes sammen. Uavhengige bevegelige deler holdes atskilt selv om de ligger tett eller er sammenflettet. Deretter genereres mesh med eksplisitt geometrisk toleranse og kjøres gjennom kontroll.

**Research.** 3MF definerer blant annet meshobjekter, komponenter og enheter. Kjernespesifikasjonen stiller krav til orientering og lukkede meshvolumer. Utvidelser og faktisk støtte i mottakende program må velges og testes, særlig ved farger og materialer. [S19][S20]

STL-eksport skal ledsages av tydelig millimeterangivelse i appen, siden enhetsforventningen må håndteres i mottakeren. 3MF-eksport skal bruke eksplisitt enhet. En 3MF-fil fra Creature Forge skal ikke utgi seg for å inneholde alle innstillingene til et bestemt slicerprosjekt hvis den bare inneholder geometri og materialreferanser.

### Kontrollrapporten

**F.** Hvert kontrollresultat får status bestått, feil eller ikke kontrollert, samt metode, terskel, revisjon og eventuelle usikkerheter. Rapporten skiller mellom:

- Topologi og geometri: åpne kanter, ikke-manifold forbindelser, selvkryssing, degenererte trekanter, orientering og uønskede løse volum.
- Dimensjoner: faktisk veggtykkelse, små detaljer og beregnet geometrifeil.
- Mekanikk: avstander, kollisjon langs definerte bevegelser og typebestemte fangstbetingelser.
- Utskrift: orientering, overheng, broer og valgt printerprofil, der analysen finnes.

Et grønt resultat betyr at angitte kontroller er bestått innenfor oppgitte forutsetninger. Det betyr ikke at programmet har bevist styrke, levetid eller utskriftssuksess for alle printere. En fast 45-gradersregel alene er ikke tilstrekkelig kontroll av støttebehov.

### Fysiske tester

En kalibreringsbrikke bør dekke flere klaringer med samme leddgeometri, materiale, laghøyde og utskriftsretning som figuren. Området 0,3–0,6 mm fra tidligere prosjektretning kan brukes som innledende forsøk, men er ikke en universell anbefalt klaring. Resultatene registreres per profil og skal ikke overføres ukritisk mellom materialer eller printere.

Foreslått testrekkefølge er enkelt ledd, to kroppsdeler og så en figur med flere ledd. Frigjøring etter print, bevegelse, fastholding og synlig svekkelse i festet vurderes separat. Antall belastningssykluser og kriterier for mekanisk styrke må bestemmes før slike egenskaper kan hevdes.

## 21 Kvalitetsmål og akseptansetester

**F.** Følgende tall er innledende målekriterier for utvikling, ikke dokumenterte egenskaper ved et ferdig program. Referansemaskiner og konkrete nettleserversjoner skal registreres før resultatene sammenlignes.

| Område | Foreslått første mål |
| --- | --- |
| Kameravisning på referanse-PC | 60 bilder per sekund ved avtalt normalmodell |
| Kameravisning på referansemobil | Minst 30 bilder per sekund ved avtalt mobilmodell |
| Synlig respons ved grep | 95-persentil under 100 ms på referanse-PC |
| Vanlig lokal sluttberegning | 95-persentil under 1 sekund på referanse-PC |
| Organisk eksportavvik | Høyst 0,05 mm for 100 mm testfigur, målt mot definert referanse |
| Avvik på mekaniske flater | Høyst 0,02 mm som innledende mål, med separat klaringsbudsjett |

Normalmodell kan innledningsvis defineres som én aktiv kroppsdel med opptil 100 000 arbeidstrekanter og en figur med ti deler og tretti lag. Mobilmodell kan innledningsvis være én aktiv del med 50 000 arbeidstrekanter. Disse profilene er testforslag som skal justeres etter måling. De er ikke grenser som allerede er godkjent eller oppnådd.

Numerisk trådlås måles separat fra produksjonsavvik. For eksempel kan testen kreve relativ lengdefeil under 10⁻⁶ for et ikke-degenerert grep. Reell printforskjell er ikke lik geometrimotorens avvik, og geometrifeil må ligge tilstrekkelig under valgt mekanisk klaring.

### Obligatoriske funksjonstester

| ID | Handling | Forventet resultat |
| --- | --- | --- |
| CF01 | Hover over grep eller velg det med berøring | Tydelig pil og aktivt påvirkningsområde |
| CF02 | Endre drapunktavstand | Ny grepfordeling uten endring av selve formen |
| CF03 | Trekk ut et punkt og et markert område | Reell lokal forming med sammenhengende tilkobling |
| CF04 | Aktiver Alt under drag | Kontinuerlig overgang til definert XY-plan uten hopp |
| CF05 | Aktiver Alt + Shift | A står fast og avstanden A–B holder låst verdi |
| CF06 | Bytt moduser flere ganger og avbryt | Ingen posisjonshopp og full gjenoppretting ved avbrudd |
| CF07 | Slipp og rediger senterlinjen | Samme form forblir redigerbar og følger linjen |
| CF08 | Bøy gjennom rette og flate kurveområder | Ingen utilsiktet vridning eller rammehopp |
| CF09 | Utvid og krymp samme område hundre ganger | Ingen topologisk korrupsjon eller ubegrenset trekantvekst |
| CF10 | Remesh en form med aktive detaljer | Fester bevares eller markeres eksplisitt som ugyldige |
| CF11 | Tegn over grunnform og tidligere blob | Både kant og innside følger samlet underlag |
| CF12 | Øk tykkelse rundt en kule | Uttrekking følger lokale normaler, ikke én global akse |
| CF13 | Endre offset med konstant tykkelse | Offset og tykkelse oppfører seg som separate parametere |
| CF14 | Lag tykk detalj i trang fordypning | Selvskjæring oppdages uten stille verdiforandring |
| CF15 | Dra en form med underformer rundt en tynn kropp | Ingen utilsiktet hopping til baksiden |
| CF16 | Slå speiling av og på etter asymmetrisk redigering | Ingen stille overskriving eller doble midtlinjeformer |
| CF17 | Flytt et hengsel med negativ form | Ny utskjæring oppstår og gammelt hull forsvinner |
| CF18 | Endre kutterens målvolumer | Bare valgte vertsvolumer påvirkes |
| CF19 | Test kollisjon mellom samplede stillinger | Kontrollmetoden oppdager eller avgrenser mellomstillingsfeil |
| CF20 | Beveg et løst ledd med flere frihetsgrader | Kontrollen bruker typekorrekt bevegelse, ikke bare én rotasjon |
| CF21 | Endre en eldre operasjon og bruk angre | Parametere, fester og avhengigheter gjenopprettes |
| CF22 | Lagre og åpne på annen støttet enhet | Redigerbar konstruksjon og enheter bevares |
| CF23 | Eksporter figur med sammenflettede deler | Volumene forblir separate og skalaen er riktig |
| CF24 | Avbryt worker eller mist grafikkontekst | Dokumentet bevares og appen kan gjenopprette visning |
| CF25 | Flytt samme sekundærform mellom flatt, rundt og smalere underlag, og endre deretter grunnformen | Sekundærformens egen geometri deformeres etter underlaget i hvert tilfelle; flytting og rotasjon av en uendret mesh er ikke tilstrekkelig |

Testene skal omfatte geometrisk måling, automatisert grensesnittbruk og menneskelig vurdering av verktøyfølelse. Bildebasert sammenligning alene er ikke tilstrekkelig. Tilfeldige redigeringssekvenser og lagrede feileksempler brukes for å oppdage sjeldne feil. Fysiske utskriftstester er et eget bevisnivå.

## 22 Åpne beslutninger med anbefalt startpunkt

Disse punktene skal ikke behandles som manglende informasjon som blokkerer hele spesifikasjonen. De er avgrensede beslutninger som må tas eller testes før den aktuelle funksjonen låses.

| Beslutning | Anbefalt startpunkt | Status |
| --- | --- | --- |
| XY ved Alt | Lokalt plan normalt på opprinnelig uttrekksretning | Må bekreftes med Sivert |
| Kurve etter første bøy | Glatt kurve med bevart rottangent og eksplisitt regel for enderetning | Betjening og geometri må testes |
| Fast tråd og buelengde | Lås A–B som avklart; buelengde er egen størrelse | Trådlås er krav, kurveregel er forslag |
| Speiling av eksisterende par | Bevar begge sider når koblingen brytes | Må bekreftes før endelig UX |
| Offsetreferanse over blobs | Samlet underlag under aktivt lag | Må bekreftes mot ønsket detaljkontroll |
| Første leddfamilie | Parametriske sammenflettede løkker for løst ledd | Tidligere retning, fysisk løsning må testes |
| Produksjonsteknologi | FDM som første verifikasjonsprofil | Praktisk forslag, ikke generell avgrensning |
| Kjernespråk og biblioteker | Wasm-kjerne med målte algoritmekandidater | Teknisk beslutning etter integrasjonstest |
| Nettlesere og referanseenheter | Chrome og Edge på Windows, Safari og Chrome på Mac, Safari på iPhone | Versjoner og kapasitet må fastsettes |
| Eksport og prosjektformat | 3MF og STL, eget redigerbart prosjektformat | Anbefaling som må bekreftes ved implementasjon |

Navnet Creature Forge, arbeidsflyten og at dette bygges fra bunnen av, er avklart. Det er ikke behov for å gjøre arkitekturvalg til brukerbeslutninger der de kan avgjøres med gode tekniske bevis. Endringer som påvirker hvordan verktøyet oppfører seg, skal derimot vurderes sammen med Sivert.

## 23 Utviklingsrekkefølge og ferdigkriterier

**F.** Implementasjonen organiseres i produksjonsrettede deler som blir stående i produktet. Et begrenset arbeidsgrensesnitt brukes først for å måle kjernen. Det skal ikke være en separat falsk demo med en annen geometrimodell.

**Trinn 1 dokument og formgrep.** Opprett prosjektmodell, transaksjoner, referansegeometri, overflatevalg, uttrekking, Alt-moduser og varige senterlinjer. Demonstrer lagring og angre fra starten. Ferdig når CF01–CF08 og relevante deler av CF21–CF22 er dokumentert.

**Trinn 2 topologi og festeoverføring.** Integrer lokal remeshing, felt for påvirkning og mekanismer for stabile fester. Ferdig når CF09–CF10 fungerer sammen med senterlinjer gjennom gjentatte redigeringer. Ingen antakelse om at dette kan utsettes til etter lagmotoren.

**Trinn 3 overflatebaserte former.** Implementer pennekurver, fylt overflateområde, lokal tykkelse, offset, avrunding, blending, underformer og overflateflytting med geometrisk deformasjon av sekundærformer. Ferdig når CF11–CF16 og CF25 er oppfylt i kombinasjon med trinn 1 og 2.

**Trinn 4 hengsler og mekanikk.** Implementer typeeditor, instanser, målrettede negative former, sammenstilling og typekorrekt bevegelseskontroll. Ferdig når CF17–CF20 består og minst én konkret leddprofil er fysisk utprøvd med registrerte forutsetninger.

**Trinn 5 eksport mobil og feilbehandling.** Fullfør produksjonskontroll, filformat, eksport, faktisk mobilbruk og gjenoppretting. Eksport og mobil testes allerede i tidligere trinn; dette trinnet lukker hele arbeidsflyten. Ferdig når CF21–CF24 og avtalte ytelsesmål er dokumentert.

Hvert trinn avsluttes med kildekode, kjørbare tester, måleresultater og kjente begrensninger i repoet. En AI-generert forklaring om at koden «bør fungere» er ikke et ferdigkriterium. Uavhengig gjennomgang av de mest krevende geometridelene anbefales før motoren brukes som produksjonsgrunnlag.

## 24 Presise beregningskontrakter

Dette kapittelet er et teknisk forslag F. Det gjør sentrale beregninger konkrete nok til å implementeres og sammenlignes. Formlene er ikke en påstand om at oppførselen er godkjent eller at løsningen allerede er robust.

### Starttilstanden til et formgrep

Ved begynnelsen av et drag lagres dokumentrevisjon, form-ID, startpunkt A, lokal normal n₀, påvirkningsområde og en referansetilstand for geometrien. To ortogonale enhetsvektorer e₁ og e₂ spenner opp det foreslåtte XY-planet, med e₁ × e₂ = n₀. Alle tre retninger står fast gjennom det aktuelle deldraget.

Et vanlig uttrekk kan skrives B = A + l · n₀, der l er valgt uttrekkslengde. Ved overgang til Alt lagres nåværende B₀, og grepet beregnes B = B₀ + x · e₁ + y · e₂. x og y kommer fra musens bevegelse omregnet til millimeter. Ved overgang til Alt + Shift lagres L = |B₀ − A|. Bare retningen oppdateres fra da av. Et nytt modusskifte lagrer nye referanseverdier uten å flytte grepet.

### Et konkret utgangspunkt for senterlinjen

For en ny enkeltbue foreslås C(s) = h₀₀(s)A + h₁₀(s)m₀ + h₀₁(s)B + h₁₁(s)m₁ for s mellom 0 og 1. Basisfunksjonene er h₀₀ = 2s³ − 3s² + 1, h₁₀ = s³ − 2s² + s, h₀₁ = −2s³ + 3s² og h₁₁ = s³ − s².

Sett m₀ = αL n₀ og m₁ = βL d, med d = (B − A)/L. α og β er eksplisitte formparametere; α = β = 1 er et foreslått første testpunkt. Dermed følger roten opprinnelig normal, enden følger grepets retning, og kurven er rett når d = n₀. Denne regelen gir en konkret bue uten å kreve at brukeren plasserer flere punkter i første drag.

Regelen må kontrolleres for nulltangent, kraftig krumning og selvkryssing. Ved senere redigering blir kurvens grep og tangenter varige dokumentdata. Hvis en bruker trenger mer enn én bue, brukes flere segmenter med definert kontinuitet. Rotens geometriske overgang til kroppen er en separat, lokal blending eller deformasjon og følger ikke automatisk bare av kurvens tangenter.

### Tverrsnitt og lokal form

Langs C transporteres en ramme med tangent T og tverrretninger U og V. Et punkt i den uttrukne formen representeres lokalt som X(s,u,v) = C(s) + uU(s) + vV(s), eventuelt med en lagret lokal restdeformasjon. Rotasjonsminimerende transport brukes som referanse for å unngå uønsket vridning. Brukerstyrt vridning lagres separat.

Dette er en egnet modell for en forlenget form med sammenhengende tverrsnitt. Et vilkårlig markert meshområde er ikke nødvendigvis et slikt volum fra starten. Da må motoren enten etablere en gyldig lokal binding med formbevarende korreksjon, eller stoppe og be om en enklere markering. Den skal ikke late som én rørformel passer alle mulige markeringer.

### Tykkelse og lagets feiltilstander

For hver prøve i et overflateområde lagres vert og lokal plassering. Bunn og topp beregnes etter offset og tykkelse i kapittel 9. Før volumet aksepteres, undersøkes foldede lokale kart, kryssende normalbaner, skjæringer i skall og forbindelsen til resten av kroppsdelen. En lokal normalberegning er en definisjon av ønsket retning, mens en egen volumkontroll avgjør om resultatet er gyldig.

Når et lag krever et lokalt overflatekart, skal kartets forvrengning og orientering kontrolleres. Et område som går rundt hele kroppen, over en fold eller over flere sømmer kan trenge flere kart. Motoren skal ikke være avhengig av at enhver tegnet form kan flates ut i ett globalt UV-plan uten feil.

### Beregningsrekkefølge for en kroppsdel

En beregningsrevisjon begynner med grunnform og dens formoperasjoner. Deretter evalueres overflatebaserte lag i avhengighetsrekkefølge. Et lag får en eksplisitt referanse til underlaget før laget selv. Additive og negative detaljoperasjoner danner kroppens redigerte volum. Hengslenes positive deler føyes til sine respektive mål, og målrettede leddkuttere trekkes fra de angitte volumene. Bevegelige motparter holdes utenfor samme union.

En endring ugyldiggjør bare avhengige resultater. Det kan likevel være nødvendig å beregne en større region når en negativ form endrer topologi. Hver kontrollrapport refererer til akkurat den revisjonen som ble evaluert. Etter en ny redigering skal rapporten vises som utdatert frem til den er kjørt igjen.

### Feilbudsjett for mekaniske klaringer

Anta at to flater er konstruert med klaring c. Hvis deres geometriske posisjonsfeil er avgrenset av εA og εB, må feilbudsjettet minst ta høyde for at faktisk klaring kan reduseres med εA + εB. Feil fra boolske operasjoner og senere forenkling må også tas med hvis de ikke allerede er inkludert i disse grensene. Fysiske utskriftsavvik kommer i tillegg og har egne profiler.

Derfor skal appen verken måle mekanisk klaring bare på en grov visningsmesh eller bruke valgt voxelstørrelse som et bevist avviksestimat. Beregningsmetoden må gi en faktisk dokumentert feilgrense eller markere resultatet som numerisk estimat.

### Kommandoer og samtidighet

Geometrikjernen trenger kontrakter for å starte et grep, oppdatere det, bekrefte eller avbryte, endre parametere, angre, evaluere dokumentet og kontrollere en revisjon. Hver forespørsel identifiserer dokument, forventet revisjon og operasjon. Resultatet identifiserer ny revisjon, berørte objekter, geometribuffere og status.

Grensesnittet kan samle raske museoppdateringer og erstatte utdaterte mellomoppdateringer. Bekreftelsen må likevel bruke brukerens siste faktiske grep. Avbrutt arbeid får ikke senere publisere en ferdig mesh som om det var gjeldende. En worker som må startes på nytt, gjenopprettes fra siste bekreftede dokumenttilstand.

## 25 Kildegrunnlag og research

Tekniske nettkilder er kontrollert 27. oktober 2026. Kildene under beskriver eksisterende metoder og API-er. Funksjonskontraktene og arkitekturforslagene i dette dokumentet er en syntese for Creature Forge, ikke påstander om at kildene allerede leverer hele produktet.

### Produktgrunnlag

P1. Avklaringene med Sivert Hovde i denne samtalen frem til bestillingen av spesifikasjonen. Dette er grunnlaget for status K og har forrang ved konflikt.

P2. Creature_Forge_til_Claude.zip, versjon 4, sist endret 23. oktober 2026. Hele tekstinnholdet i de fem Markdown-dokumentene er gjennomgått. Pakken inneholder også tolv SVG-funksjonsikoner, men ingen ferdig geometrimotor. Den eldre føringen om obligatorisk start med hode, bare forhåndsdefinerte fester, eksisterende repo og en enkel første prototype er erstattet av nyere avklaringer.

### Primære tekniske kilder

S1. CGAL 6.2. Meshing and Remeshing of Polygonal Meshes. Lokale og globale metoder, topologi og tetthet. https://doc.cgal.org/6.2/PMP_Remeshing/index.html

S2. Wang, Jüttler, Zheng og Liu. Computation of Rotation Minimizing Frames. ACM Transactions on Graphics, 2008. Referanse for transport av tverrsnitt langs kurver. https://www.microsoft.com/en-us/research/publication/computation-rotation-minimizing-frames/

S3. libigl. Tutorial, Shape Deformation. Deformasjoner med grep, ARAP og elastisk forming. https://libigl.github.io/tutorial/

S4. Blender. Solidify Modifier, Known Limitations. Begrensninger ved tykkelse og normalforskyvning. https://docs.blender.org/manual/en/5.3/modeling/modifiers/generate/solidify.html

S5. Geometry Central. Tracing Geodesic Paths. Sporing av retning langs overflater. https://geometry-central.net/surface/algorithms/geodesic_paths/

S6. Geometry Central. Vector Heat Method. Transport av tangentretninger og lokale kart. https://geometry-central.net/surface/algorithms/vector_heat_method/

S7. libigl. swept_volume_signed_distance. Diskret samplet bevegelsesvolum. https://libigl.github.io/dox/swept__volume__signed__distance_8h.html

S8. Flexible Collision Library. Offisiell prosjektbeskrivelse. Kollisjon, avstand og toleranse. https://github.com/flexible-collision-library/fcl

S9. OpenVDB. Cookbook. Sparsomme volum og level set-boolske operasjoner. https://www.openvdb.org/documentation/doxygen/codeExamples.html

S10. Manifold. Using Manifold. WebAssembly-integrasjon og eksplisitt minnehåndtering. https://manifoldcad.org/docs/jsapi/documents/Using_Manifold.html

S11. CGAL 6.1. Polygon Mesh Processing, Corefinement and Boolean Operations. Robusthetsbetingelser ved gjentatte operasjoner. https://doc.cgal.org/6.1/Polygon_mesh_processing/index.html

S12. wasm-bindgen. Introduction. Kobling mellom Rust, Wasm og JavaScript. https://rustwasm.github.io/docs/wasm-bindgen/introduction.html

S13. MDN. Transferable objects. Eierskap ved overføring av buffere til workers. https://developer.mozilla.org/en-US/docs/Web/API/Web_Workers_API/Transferable_objects

S14. Emscripten. Pthreads support. Forutsetninger for delt minne og tråder i nettleseren. https://emscripten.org/docs/porting/pthreads.html

S15. Three.js. WebGPURenderer. WebGPU og WebGL 2 som visningsbakender. https://threejs.org/docs/pages/WebGPURenderer.html

S16. MDN. WebGPU API. Tilgjengelighet, sikker kontekst og grafikkressurser. https://developer.mozilla.org/en-US/docs/Web/API/WebGPU_API

S17. MDN. IndexedDB API. Asynkron lokal lagring av strukturert data. https://developer.mozilla.org/en-US/docs/Web/API/IndexedDB_API

S18. MDN. Storage quotas and eviction criteria. Kvoter og varighet for nettleserlagring. https://developer.mozilla.org/en-US/docs/Web/API/Storage_API/Storage_quotas_and_eviction_criteria

S19. 3MF Consortium. Specification Suite. Kjerneformat og separate utvidelser. https://3mf.io/spec/

S20. 3MF Consortium. Core Specification 1.3.0. Meshkrav, orientering og objektstruktur. https://3mf.io/wp-content/uploads/sites/106/2025/02/3MF_Core_Specification_v1.3.0.pdf

### Biblioteker og lisensvalg

Bibliotekene er kandidater som må vurderes for integrasjon, vedlikehold, Wasm-bygg, minnebruk og lisens. CGALs remeshingpakke oppgir GPL, og CGAL tilbyr også kommersiell lisens. Manifold oppgir Apache-2.0. Det er ikke tatt en endelig beslutning om å inkludere noen av dem i Creature Forge. [S21][S22]

S21. CGAL. Lisensoversikt og pakkelisens for remeshing. https://www.cgal.org/license.html og https://doc.cgal.org/6.2/PMP_Remeshing/group__PkgPMPRemeshingRef.html

S22. Manifold. Offisielt kildekoderepo med lisens og prosjektbeskrivelse. https://github.com/elalish/manifold
