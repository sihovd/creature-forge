import { expect, test, type Page } from '@playwright/test';
import { closeLayersOnMobile, layerRow, newSpline, viewport } from './helpers';

const shape = (page: Page, id: number) => page.evaluate((s) => window.creatureForgeTest!.shapeInfo(s), id);

// Negativ form (issue #12 punkt 4): «＋ Legg til» / «− Trekk fra» på lagraden,
// eksplisitte kuttemål, og visning av kutteren skilt fra om den skjærer.
test('et lag trekkes fra et annet og kan slås av, skjules og angres', async ({ page, isMobile }) => {
  await page.goto('/');
  await newSpline(page);
  await page.locator('#thickness').fill('4');
  await newSpline(page);
  await expect(viewport(page)).toHaveAttribute('data-shape-count', '2');
  const whole = (await shape(page, 1)).triangles;
  expect(whole).toBeGreaterThan(100);

  // Lag 2 gjøres om til en negativ form med knappen på lagraden.
  const row = await layerRow(page, 'l2');
  await row.locator('[data-action="cut"]').click();
  await expect(viewport(page)).toHaveAttribute('data-cutters', '1');
  await expect(row.locator('.cut-badge')).toHaveText('− 1');
  // Den tynnere kutteren ligger langs samme akse og skjærer et hull gjennom lag 1.
  await expect.poll(async () => (await shape(page, 1)).triangles).not.toBe(whole);
  const cut = (await shape(page, 1)).triangles;
  // Kutteren er fortsatt et eget lag og vises.
  expect((await shape(page, 2)).visible).toBe(true);

  // Inspektøren: kuttemålet er valgt, og utskjæringen kan slås av uten å skjule kutteren.
  await row.click();
  await closeLayersOnMobile(page, isMobile);
  await expect(page.locator('#cut-subtract')).toHaveAttribute('aria-pressed', 'true');
  await expect(page.locator('#cut-targets input[data-target="1"]')).toBeChecked();
  await page.locator('#cut-active').uncheck();
  await expect.poll(async () => (await shape(page, 1)).triangles).toBe(whole);
  expect((await shape(page, 2)).visible).toBe(true);
  await page.locator('#cut-active').check();
  await expect.poll(async () => (await shape(page, 1)).triangles).toBe(cut);

  // «Vis kutter» av skjuler kutteren, men kuttet står.
  await page.locator('#cut-show').uncheck();
  await expect.poll(async () => (await shape(page, 2)).visible).toBe(false);
  expect((await shape(page, 1)).triangles).toBe(cut);
  await page.locator('#cut-show').check();

  // Fjernes målet fra listen, er lag 1 helt igjen. Angre setter kuttet tilbake.
  await page.locator('#cut-targets input[data-target="1"]').uncheck();
  await expect.poll(async () => (await shape(page, 1)).triangles).toBe(whole);
  await page.getByRole('button', { name: 'Angre' }).click();
  await expect.poll(async () => (await shape(page, 1)).triangles).toBe(cut);

  // Tilbake til «＋ Legg til».
  await page.locator('#cut-add').click();
  await expect(viewport(page)).toHaveAttribute('data-cutters', '0');
  await expect.poll(async () => (await shape(page, 1)).triangles).toBe(whole);
});
