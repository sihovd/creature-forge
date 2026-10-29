import { expect, test, type Page } from '@playwright/test';
import { anchorScreen, curveScreen, newSpline, status, touchDrag, viewport } from './helpers';

test.skip(({ isMobile }) => !isMobile, 'Berøring testes på mobil');

const length = (page: Page, s: number) => page.evaluate((id) => window.creatureForgeTest!.splineLength(id), s) as Promise<number>;

test('dra tuppen ut med finger, og langt trykk åpner hurtigmenyen', async ({ page }) => {
  await page.goto('/');
  await newSpline(page);
  const tip = await anchorScreen(page, 1, -1);
  await touchDrag(page, tip, { x: tip.x, y: tip.y - 40 });
  await expect.poll(() => length(page, 1)).toBeGreaterThan(44);
  // Langt trykk på kurven: hurtigmenyen, uten endring.
  const before = await length(page, 1);
  const mid = await curveScreen(page, 1, 0.5);
  await touchDrag(page, mid, mid, 1, 800);
  await expect(page.locator('.context-menu')).toBeVisible();
  expect(await length(page, 1)).toBeCloseTo(before, 6);
});

test('Bézier-verktøyet setter inn et punkt med et trykk', async ({ page }) => {
  await page.goto('/');
  await newSpline(page);
  await page.getByRole('button', { name: 'Bézier' }).click();
  const p = await curveScreen(page, 1, 0.45);
  await page.touchscreen.tap(p.x, p.y);
  await expect(status(page)).toContainText('Satte inn et ankerpunkt');
  expect((await page.evaluate(() => window.creatureForgeTest!.splineAnchorsWorld(1)))!.length).toBe(3);
});

test('konturpunkt dras med finger i «Rediger kontur»', async ({ page }) => {
  await page.goto('/');
  await newSpline(page);
  await page.locator('#thickness').fill('15');
  await page.locator('#thickness').press('Enter');
  await expect(status(page)).toContainText('Tykkelse 15 mm');
  await page.getByRole('button', { name: 'Ny kontur' }).click();
  await page.getByRole('button', { name: 'Rediger kontur på modellen' }).tap();
  await expect(viewport(page)).toHaveAttribute('data-contour-mode', 'on');
  const revision = Number(await viewport(page).getAttribute('data-revision'));
  const b = (await page.locator('.profile-svg.on-model .point').first().boundingBox())!;
  const from = { x: b.x + b.width / 2, y: b.y + b.height / 2 };
  await touchDrag(page, from, { x: from.x + 30, y: from.y });
  await expect(viewport(page)).toHaveAttribute('data-revision', String(revision + 1));
  await expect(status(page)).toContainText('Flyttet konturpunkt');
});
