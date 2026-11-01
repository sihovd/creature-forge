// Lager bildene av grunnformene i paletten (public/shapes/*.png).
// Kjør `npm run dev` i en annen terminal først, deretter `node scripts/thumbnails.mjs`.
// Bildene tegnes med src/thumbnails.ts i nettleseren og lagres som filer, så
// appen slipper å tegne dem hver gang den starter.
import { writeFile } from 'node:fs/promises';
import { chromium } from 'playwright';

const url = process.env.DEV_URL ?? 'http://localhost:5173/';
const browser = await chromium.launch();
const page = await browser.newPage();
await page.goto(url);
const images = await page.evaluate(async () => {
  const { renderThumbnails } = await import('/src/thumbnails.ts');
  return Object.fromEntries(renderThumbnails(['sphere', 'capsule', 'cone', 'box'], 420, 300));
});
for (const [kind, data] of Object.entries(images)) {
  await writeFile(new URL(`../public/shapes/${kind}.png`, import.meta.url), Buffer.from(data.split(',')[1], 'base64'));
  console.log(`public/shapes/${kind}.png`);
}
await browser.close();
