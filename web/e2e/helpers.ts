import { expect, type Page } from '@playwright/test';

export const viewport = (page: Page) => page.locator('#viewport');
export const status = (page: Page) => page.locator('#status');

/** «Ny spline» og retning i aksekorset. Lager én ny spline. */
export async function newSpline(page: Page, axis = '+Y') {
  const button = page.getByRole('button', { name: 'Ny spline' });
  await expect(button).toBeEnabled();
  const before = Number(await viewport(page).getAttribute('data-splines'));
  await button.click();
  await page.locator('.axis-label', { hasText: axis }).click();
  await expect(viewport(page)).toHaveAttribute('data-splines', String(before + 1));
}

export const anchorsWorld = (page: Page, spline: number) =>
  page.evaluate((s) => window.creatureForgeTest!.splineAnchorsWorld(s), spline) as Promise<[number, number, number][]>;

export async function anchorScreen(page: Page, spline: number, index: number) {
  const p = await page.evaluate(([s, i]) => window.creatureForgeTest!.splineAnchor(s, i), [spline, index]);
  expect(p).not.toBeNull();
  return p!;
}

export async function curveScreen(page: Page, spline: number, fraction: number) {
  const p = await page.evaluate(([s, f]) => window.creatureForgeTest!.splineCurvePoint(s, f), [spline, fraction]);
  expect(p).not.toBeNull();
  return p!;
}

/** Raden med nøkkelen `key` i laglisten (l = lag, s = spline, c = kontur). Åpner listen ved behov. */
export async function layerRow(page: Page, key: string) {
  const toggle = page.locator('#toggle-layers');
  if ((await toggle.getAttribute('aria-expanded')) !== 'true') await toggle.click();
  return page.locator(`#layer-list [data-key="${key}"] > .row`);
}

/** Lukker laglisten på små skjermer, der den ligger over arbeidsflaten. */
export async function closeLayersOnMobile(page: Page, isMobile: boolean) {
  const toggle = page.locator('#toggle-layers');
  if (isMobile && (await toggle.getAttribute('aria-expanded')) === 'true') await toggle.click();
}

/** «Ny gren» for valgt spline, og godta foreslått retning med et trykk på retningshåndtaket. */
export async function newBranch(page: Page) {
  const before = Number(await viewport(page).getAttribute('data-splines'));
  await page.getByRole('button', { name: 'Ny gren', exact: true }).click();
  await page.locator('.axis-free').click();
  await expect(viewport(page)).toHaveAttribute('data-splines', String(before + 1));
}
