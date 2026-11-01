import { expect, test, type Page } from '@playwright/test';
import { anchorScreen, anchorsWorld, curveScreen, newBranch, newSpline, status, viewport } from './helpers';

type V = [number, number, number];
const dist = (a: V, b: V) => Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);
const info = (page: Page, s: number) => page.evaluate((id) => window.creatureForgeTest!.splineInfo(id), s);

async function drag(page: Page, from: { x: number; y: number }, to: { x: number; y: number }) {
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(to.x, to.y, { steps: 10 });
  await page.mouse.up();
}

test.describe('med mus', () => {
  test.skip(({ isMobile }) => isMobile, 'Drag med mus');

  test('enden av en gren kobles tilbake til morsplinen og følger den (nett)', async ({ page }) => {
    await page.goto('/');
    await newSpline(page);
    await newBranch(page);
    const target = await curveScreen(page, 1, 0.7);
    await drag(page, await anchorScreen(page, 2, -1), target);
    await expect(viewport(page)).toHaveAttribute('data-ends', '1');
    expect(await info(page, 2)).toMatchObject({ endParent: 1 });
    const tip = (await anchorsWorld(page, 2)).at(-1)!;
    // Morsplinen trekkes lenger ut: enden av grenen følger etter.
    const rootTip = await anchorScreen(page, 1, -1);
    await drag(page, rootTip, { x: rootTip.x, y: rootTip.y - 60 });
    await expect(status(page)).toContainText('Trekk ut');
    await expect.poll(async () => dist((await anchorsWorld(page, 2)).at(-1)!, tip)).toBeGreaterThan(1);
    expect(await info(page, 2)).toMatchObject({ endParent: 1 });
    // Angre løsner ikke noe annet enn siste steg.
    await page.getByRole('button', { name: 'Angre' }).click();
    await expect.poll(async () => dist((await anchorsWorld(page, 2)).at(-1)!, tip)).toBeLessThan(1e-6);
  });

  test('en kontur dras langs splinen og over på en annen spline', async ({ page }) => {
    await page.goto('/');
    await newSpline(page);
    await newBranch(page);
    const c = await curveScreen(page, 1, 0.2);
    await page.mouse.click(c.x, c.y);
    await expect(viewport(page)).toHaveAttribute('data-selection', 'spline:1');
    await page.getByRole('button', { name: 'Ny kontur' }).click();
    await expect(viewport(page)).toHaveAttribute('data-profiles', '1');
    await page.locator('#station-at').fill('15');
    await page.locator('#station-at').press('Enter');
    await expect.poll(async () => (await page.evaluate(() => window.creatureForgeTest!.profileAts(1)))![0]).toBeCloseTo(0.15, 2);
    const ring = (await page.evaluate(() => window.creatureForgeTest!.ringScreen(1, 0, 0)))!;
    await drag(page, ring, await curveScreen(page, 2, 0.7));
    await expect.poll(() => page.evaluate(() => window.creatureForgeTest!.profileAts(2))).toHaveLength(1);
    expect(await page.evaluate(() => window.creatureForgeTest!.profileAts(1))).toHaveLength(0);
    await expect(viewport(page)).toHaveAttribute('data-selection', 'contour:2:0');
    await expect(page.locator('#layer-list [data-key="c2:0"]')).toHaveCount(1);
  });

  test('et punkt flyttes fritt, og med Alt i xyz-planet som vender mot kameraet', async ({ page }) => {
    await page.goto('/');
    await newSpline(page);
    await page.getByRole('button', { name: 'Bézier' }).click();
    const mid = await curveScreen(page, 1, 0.5);
    await page.mouse.click(mid.x, mid.y);
    await expect.poll(async () => (await anchorsWorld(page, 1)).length).toBe(3);
    await page.getByRole('button', { name: 'Velg' }).click();
    const before = (await anchorsWorld(page, 1))[1];
    const p = await anchorScreen(page, 1, 1);
    await page.keyboard.down('Alt');
    await drag(page, p, { x: p.x + 50, y: p.y - 30 });
    await page.keyboard.up('Alt');
    const after = (await anchorsWorld(page, 1))[1];
    // Kameraet ser mest langs z, så punktet flyttes i XY-planet.
    expect(dist(after, before)).toBeGreaterThan(3);
    expect(after[2]).toBeCloseTo(before[2], 6);
    // Uten Alt flyttes punktet fritt i synsplanet, også i dybden.
    const q = await anchorScreen(page, 1, 1);
    await drag(page, q, { x: q.x - 40, y: q.y + 20 });
    await expect.poll(async () => Math.abs((await anchorsWorld(page, 1))[1][2] - after[2])).toBeGreaterThan(0.5);
  });
});

test('mesh-deler settes inn fra registeret, og egne OBJ-filer kan importeres', async ({ page }) => {
  await page.goto('/');
  await newSpline(page);
  await page.locator('#part-source').selectOption({ label: 'Horn' });
  const before = Number(await viewport(page).getAttribute('data-revision'));
  await page.getByRole('button', { name: 'Sett inn', exact: true }).click();
  await expect(viewport(page)).toHaveAttribute('data-parts', '1');
  await expect(page.locator('.part-row strong')).toHaveText('Horn');
  expect(Number(await viewport(page).getAttribute('data-revision'))).toBeGreaterThan(before);
  await page.locator('.part-row input[type=number]').nth(1).fill('20');
  await page.locator('.part-row input[type=number]').nth(1).press('Enter');
  await expect(status(page)).toContainText('Høyde 20 mm');
  const obj = 'v 0 0 0\nv 1 0 0\nv 0 0 1\nv 0 2 0\nf 1 3 2\nf 1 2 4\nf 1 4 3\nf 2 3 4\n';
  await page.locator('#part-file').setInputFiles({ name: 'Finne.obj', mimeType: 'text/plain', buffer: Buffer.from(obj) });
  await expect(page.locator('#part-source option', { hasText: 'Finne (importert)' })).toHaveCount(1);
  await expect(page.locator('#part-source')).toHaveValue('s:1');
  await page.locator('#tool-part').click();
  await expect(viewport(page)).toHaveAttribute('data-parts', '2');
});

test('speilmodus og meshinnstillinger kan slås av og på og angres', async ({ page }) => {
  await page.goto('/');
  await newSpline(page, '+X');
  await page.locator('#tool-mirror').click();
  await expect(viewport(page)).toHaveAttribute('data-mirror', 'on');
  await expect(page.locator('#tool-mirror')).toHaveAttribute('aria-pressed', 'true');
  await page.locator('#open-settings').click();
  await expect(page.locator('#settings')).toBeVisible();
  await page.locator('#crease-detail').selectOption({ label: 'Av' });
  await expect(viewport(page)).toHaveAttribute('data-crease-detail', '0');
  await page.getByRole('button', { name: 'Lukk' }).click();
  await expect(page.locator('#settings')).toBeHidden();
  await page.getByRole('button', { name: 'Angre' }).click();
  await expect(viewport(page)).toHaveAttribute('data-crease-detail', '2');
  await page.getByRole('button', { name: 'Angre' }).click();
  await expect(viewport(page)).toHaveAttribute('data-mirror', '');
});
