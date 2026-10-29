import { expect, test, type Page } from '@playwright/test';
import { curveScreen, newSpline, status, viewport } from './helpers';

const ats = (page: Page, s = 1) => page.evaluate((id) => window.creatureForgeTest!.profileAts(id), s) as Promise<number[]>;
const ring = async (page: Page, station: number, i: number) => {
  const p = await page.evaluate(([st, k]) => window.creatureForgeTest!.ringScreen(1, st, k), [station, i]);
  expect(p).not.toBeNull();
  return p!;
};
const camera = (page: Page) => page.evaluate(() => window.creatureForgeTest!.cameraPosition());
const menu = (page: Page) => page.locator('.context-menu');
const overlayPoint = (page: Page, i: number) => page.locator('.profile-svg.on-model .point').nth(i);
const center = async (page: Page, i: number) => {
  const b = (await overlayPoint(page, i).boundingBox())!;
  return { x: b.x + b.width / 2, y: b.y + b.height / 2 };
};

/** En tykk spline med en kontur der det ble høyreklikket. */
async function withContour(page: Page, at = 0.5) {
  await page.goto('/');
  await newSpline(page);
  await page.locator('#thickness').fill('15');
  await page.locator('#thickness').press('Enter');
  await expect(status(page)).toContainText('Tykkelse 15 mm');
  const p = await curveScreen(page, 1, at);
  await page.mouse.click(p.x, p.y, { button: 'right' });
  await menu(page).getByRole('menuitem', { name: 'Ny kontur her' }).click();
  await expect(viewport(page)).toHaveAttribute('data-profiles', '1');
}

async function openEditor(page: Page) {
  const q = await ring(page, 0, 0);
  await page.mouse.click(q.x, q.y, { button: 'right' });
  await menu(page).getByRole('menuitem', { name: 'Rediger kontur' }).first().click();
  await expect(viewport(page)).toHaveAttribute('data-contour-mode', 'on');
}

/** Punkt 0 i editoren ligger oppå punkt 0 i omrisset på modellen. */
async function expectAligned(page: Page) {
  const a = await center(page, 0);
  const r = await ring(page, 0, 0);
  expect(Math.hypot(a.x - r.x, a.y - r.y)).toBeLessThan(2);
}

test.describe('med mus', () => {
  test.skip(({ isMobile }) => isMobile, 'Mus');

  test('«Ny kontur her» legger konturen der det ble trykket, og den kan dras langs splinen', async ({ page }) => {
    await withContour(page, 0.3);
    const [at] = await ats(page);
    expect(at).toBeGreaterThan(0.2);
    expect(at).toBeLessThan(0.4);
    // Dra konturen oppover langs splinen.
    const q = await ring(page, 0, 0);
    const up = await curveScreen(page, 1, 0.75);
    await page.mouse.move(q.x, q.y);
    await page.mouse.down();
    await page.mouse.move(q.x, up.y, { steps: 6 });
    await expect(status(page)).toContainText('Flytt kontur 1 langs splinen');
    await page.mouse.up();
    await expect.poll(async () => (await ats(page))[0]).toBeGreaterThan(0.6);
    await page.keyboard.press('Control+Z');
    await expect.poll(async () => (await ats(page))[0]).toBeCloseTo(at, 6);
  });

  test('et vanlig trykk velger konturen uten å åpne redigeringen', async ({ page }) => {
    await withContour(page);
    await page.keyboard.press('Escape');
    await page.locator('#layer-list [data-key="s1"] > .row').click();
    const q = await ring(page, 0, 0);
    await page.mouse.click(q.x, q.y);
    await expect(viewport(page)).toHaveAttribute('data-selection', 'contour:1:0');
    await expect(viewport(page)).not.toHaveAttribute('data-contour-mode', 'on');
  });

  test('«Rediger kontur» på modellen: vinkelrett kamera, konkav H, punkter, zoom og Ferdig', async ({ page }) => {
    await withContour(page);
    await page.locator('#registry [data-key="p:H"] > button').click();
    await expect(status(page)).toContainText('Kontur: H');
    const before = await camera(page);
    await openEditor(page);
    await expect(page.locator('.contour-bar')).toContainText('Rediger kontur 1');
    await expect(page.locator('.profile-svg.on-model .point')).toHaveCount(12);
    // Kameraet ser langs splinen (loddrett), og editoren ligger oppå omrisset.
    const cam = await camera(page);
    const ringWorld = await page.evaluate(() => window.creatureForgeTest!.splineAnchorsWorld(1));
    expect(Math.abs(cam[0] - ringWorld![0][0])).toBeLessThan(1e-3);
    expect(Math.abs(cam[2] - ringWorld![0][2])).toBeLessThan(1e-3);
    await expectAligned(page);

    // Dra et indre hjørne i H-en innover: profilen forblir konkav og lagres.
    const revision = Number(await viewport(page).getAttribute('data-revision'));
    const p = await center(page, 3);
    await page.mouse.move(p.x, p.y);
    await page.mouse.down();
    await page.mouse.move(p.x - 8, p.y + 6, { steps: 4 });
    await page.mouse.up();
    await expect(viewport(page)).toHaveAttribute('data-revision', String(revision + 1));
    await expect(status(page)).toContainText('Flyttet konturpunkt');
    await expect(page.locator('.profile-svg.on-model .point')).toHaveCount(12);

    // Mykt punkt gir håndtak.
    await page.locator('.contour-bar').getByRole('button', { name: 'Mykt punkt' }).click();
    await expect(status(page)).toContainText('Mykt punkt i konturen');
    await expect(page.locator('.profile-svg.on-model .handle')).not.toHaveCount(0);

    // Sett inn et punkt med et trykk på omrisset (midt på ytterkanten til høyre).
    await page.locator('.contour-bar').getByRole('button', { name: 'Sett inn punkt' }).click();
    const a = await center(page, 0);
    const b = await center(page, 1);
    await page.mouse.click((a.x + b.x) / 2, (a.y + b.y) / 2);
    await expect(page.locator('.profile-svg.on-model .point')).toHaveCount(13);

    // Zoom med scroll: editoren følger modellen.
    await page.mouse.move(a.x + 40, a.y);
    await page.mouse.wheel(0, -300);
    await page.waitForTimeout(400);
    await expectAligned(page);

    // Ferdig: tilbake til vanlig visning med kameraet der det var.
    await page.locator('.contour-bar').getByRole('button', { name: 'Ferdig' }).click();
    await expect(viewport(page)).not.toHaveAttribute('data-contour-mode', 'on');
    const after = await camera(page);
    after.forEach((v, i) => expect(v).toBeCloseTo(before[i], 3));
    await expect(viewport(page)).toHaveAttribute('data-profiles', '1');
    // Endringene i modusen kan angres.
    await page.keyboard.press('Control+Z');
    await expect(status(page)).toContainText('Angret');
  });

  test('en kontur i et låst lag kan ikke redigeres på modellen', async ({ page }) => {
    await withContour(page);
    await page.locator('#layer-list [data-key="l1"] > .row').getByRole('button', { name: 'Lås laget' }).click();
    await expect(viewport(page)).toHaveAttribute('data-locked-layers', '1');
    await page.locator('#layer-list [data-key="c1:0"] > .row').click();
    await expect(page.getByRole('button', { name: 'Rediger kontur på modellen' })).toBeDisabled();
  });
});

test('langt trykk på en kontur gir «Rediger kontur» på touch', async ({ page, isMobile }) => {
  test.skip(!isMobile, 'Berøring testes på mobil');
  await page.goto('/');
  await newSpline(page);
  await page.locator('#thickness').fill('15');
  await page.locator('#thickness').press('Enter');
  await expect(status(page)).toContainText('Tykkelse 15 mm');
  await page.getByRole('button', { name: 'Ny kontur' }).click();
  await expect(viewport(page)).toHaveAttribute('data-profiles', '1');
  const q = await ring(page, 0, 0);
  const canvas = page.locator('#viewport canvas');
  const init = { pointerType: 'touch', pointerId: 9, isPrimary: true, button: 0, buttons: 1, clientX: q.x, clientY: q.y, bubbles: true };
  await canvas.dispatchEvent('pointerdown', init);
  await page.waitForTimeout(800);
  await canvas.dispatchEvent('pointerup', { ...init, buttons: 0 });
  await expect(menu(page)).toBeVisible();
  await menu(page).getByRole('menuitem', { name: 'Rediger kontur' }).first().tap();
  await expect(viewport(page)).toHaveAttribute('data-contour-mode', 'on');
  // Panelet er skjult på små skjermer, og «Ferdig» er synlig og kan trykkes.
  await expect(page.locator('.panel')).toBeHidden();
  await page.locator('.contour-bar').getByRole('button', { name: 'Ferdig' }).tap();
  await expect(viewport(page)).not.toHaveAttribute('data-contour-mode', 'on');
});
