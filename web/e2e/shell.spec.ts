import { readFile } from 'node:fs/promises';
import { expect, test } from '@playwright/test';
import { status, viewport } from './helpers';

// Rammen rundt arbeidsflaten etter Siverts skisse: palett med grunnformer,
// toppfelt med prosjektnavn, eksport og tema, og tomme tilstander.

test('tomme tilstander i lagpanelet og inspektøren', async ({ page }) => {
  await page.goto('/');
  await expect(page.locator('#layers-empty')).toBeVisible();
  await expect(page.locator('#inspector-empty')).toBeVisible();
  await expect(page.locator('#inspector-head')).toBeHidden();
  await expect(page.getByRole('button', { name: 'Eksporter', exact: true })).toBeDisabled();
});

test('grunnformene i paletten er spline-former, og hver er ett angresteg', async ({ page }) => {
  await page.goto('/');
  const cards = { Kule: 1, Kapsel: 2, Kjegle: 3, Boks: 4 };
  for (const [name, count] of Object.entries(cards)) {
    const card = page.locator('.shape-card', { hasText: name });
    await expect(card).toBeEnabled();
    await card.click();
    await expect(viewport(page)).toHaveAttribute('data-layers', String(count));
    await expect(viewport(page)).toHaveAttribute('data-revision', String(count));
    await expect(page.locator('#layer-list')).toContainText(name);
  }
  // Hver form er én rotspline med mesh rundt.
  await expect(viewport(page)).toHaveAttribute('data-splines', '4');
  await expect(viewport(page)).toHaveAttribute('data-shape-count', '4');
  await expect(page.locator('#layers-empty')).toBeHidden();
  await expect(page.locator('#empty-hint')).toBeHidden();

  // Kjegle og boks har konturer og flate ender; kula og kapselen har ingen konturer.
  const info = await page.evaluate(() => [1, 2, 3, 4].map((s) => window.creatureForgeTest!.profileAts(s)?.length));
  expect(info).toEqual([0, 0, 5, 2]);

  await page.getByRole('button', { name: 'Angre' }).click();
  await expect(viewport(page)).toHaveAttribute('data-layers', '3');
  await expect(viewport(page)).toHaveAttribute('data-splines', '3');
});

test('Eksporter laster ned en binær STL av de synlige formene', async ({ page }) => {
  await page.goto('/');
  await page.locator('.shape-card', { hasText: 'Kule' }).click();
  await expect(viewport(page)).toHaveAttribute('data-shape-count', '1');
  const exportButton = page.getByRole('button', { name: 'Eksporter', exact: true });
  await expect(exportButton).toBeEnabled();
  const download = page.waitForEvent('download');
  await exportButton.click();
  expect((await download).suggestedFilename()).toBe('skapning.stl');
  const bytes = await readFile((await (await download).path())!);
  const triangles = bytes.readUInt32LE(80);
  expect(triangles).toBeGreaterThan(100);
  expect(bytes.length).toBe(84 + 50 * triangles);
  await expect(status(page)).toContainText('Eksporterte skapning.stl');
});

test('prosjektnavnet brukes i filnavnet', async ({ page }) => {
  await page.goto('/');
  await page.locator('.shape-card', { hasText: 'Kapsel' }).click();
  await expect(viewport(page)).toHaveAttribute('data-shape-count', '1');
  const name = page.getByRole('textbox', { name: 'Navn på skapningen' });
  await name.fill('Liten drage');
  await name.press('Enter');
  const download = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Lagre', exact: true }).click();
  expect((await download).suggestedFilename()).toBe('liten-drage.cfproj');
});

test('mørkt tema huskes mellom besøk', async ({ page }) => {
  await page.goto('/');
  await expect(page.locator('html')).not.toHaveAttribute('data-theme', 'dark');
  await page.locator('#toggle-theme').click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
});

test('fanene i paletten bytter innhold', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('tab', { name: 'Kontroll' }).click();
  await expect(page.locator('#spacing')).toBeVisible();
  await expect(page.locator('.shape-cards')).toBeHidden();
  await page.getByRole('tab', { name: 'Detaljer' }).click();
  await expect(page.locator('.detail-card').first()).toBeVisible();
  await page.getByRole('tab', { name: 'Grunnformer' }).click();
  await expect(page.locator('.shape-cards')).toBeVisible();
});

test.describe('kamera', () => {
  test.skip(({ isMobile }) => isMobile, 'Samme knapper på iPad');

  test('visningskuben og zoomknappene flytter kameraet', async ({ page }) => {
    await page.goto('/');
    await page.locator('.shape-card', { hasText: 'Kule' }).click();
    await expect(viewport(page)).toHaveAttribute('data-shape-count', '1');
    const camera = () => page.evaluate(() => window.creatureForgeTest!.cameraOffset());
    const start = await camera();
    await page.getByRole('button', { name: 'Zoom inn' }).click();
    await expect.poll(async () => Math.hypot(...(await camera()))).toBeLessThan(Math.hypot(...start) - 5);

    // Midt på kubens øvre side gir visning rett ovenfra.
    const cube = await page.locator('#view-cube').boundingBox();
    const top = await page.evaluate(() => window.creatureForgeTest!.viewCubeFace('y'));
    await page.mouse.click(cube!.x + top!.x, cube!.y + top!.y);
    await expect.poll(async () => {
      const [x, , z] = await camera();
      return Math.hypot(x, z);
    }).toBeLessThan(2);
  });
});
