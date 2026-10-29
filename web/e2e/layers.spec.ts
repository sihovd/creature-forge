import { expect, test, type Page } from '@playwright/test';
import { anchorScreen, anchorsWorld, closeLayersOnMobile, curveScreen, layerRow, newBranch, newSpline, status, viewport } from './helpers';

const length = (page: Page, spline: number) =>
  page.evaluate((s) => window.creatureForgeTest!.splineLength(s), spline) as Promise<number>;
const row = (page: Page, key: string) => ({
  click: async () => (await layerRow(page, key)).click(),
  dblclick: async () => (await layerRow(page, key)).dblclick(),
  getByRole: (role: 'button', o: { name: string }) => ({ click: async () => (await layerRow(page, key)).getByRole(role, o).click() }),
  locator: () => layerRow(page, key),
});
const expectRow = async (page: Page, key: string) => expect(await layerRow(page, key));
const dist = (a: number[], b: number[]) => Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);

test('ny spline får retningen som velges i aksekorset', async ({ page }) => {
  await page.goto('/');
  await newSpline(page, '+X');
  let a = await anchorsWorld(page, 1);
  expect(a[1][0] - a[0][0]).toBeCloseTo(40, 6);
  expect(a[1][1] - a[0][1]).toBeCloseTo(0, 6);
  await newSpline(page, '−Z');
  a = await anchorsWorld(page, 2);
  expect(a[1][2] - a[0][2]).toBeCloseTo(-40, 6);
  // Avbryt lager ingenting.
  await page.getByRole('button', { name: 'Ny spline' }).click();
  await page.locator('.gizmo-bar').getByRole('button', { name: 'Avbryt' }).click();
  await expect(page.locator('.axis-label').first()).toBeHidden();
  await expect(viewport(page)).toHaveAttribute('data-splines', '2');
});

test('laglisten viser lag, grener og konturer, og valget samsvarer med 3D', async ({ page, isMobile }) => {
  await page.goto('/');
  await newSpline(page);
  await newSpline(page, '+X');
  await (await expectRow(page, 'l1')).toContainText('Lag 1');
  await (await expectRow(page, 'l2')).toContainText('Lag 2');
  // Velg hovedsplinen i lag 1 i listen og lag en gren og en kontur.
  await row(page, 's1').click();
  await expect(viewport(page)).toHaveAttribute('data-selection', 'spline:1');
  await newBranch(page);
  await expect(viewport(page)).toHaveAttribute('data-splines', '3');
  await (await expectRow(page, 's3')).toContainText('Gren 3');
  await row(page, 's1').click();
  await page.getByRole('button', { name: 'Ny kontur' }).click();
  await (await expectRow(page, 'c1:0')).toContainText('Kontur 1');
  await expect(viewport(page)).toHaveAttribute('data-selection', 'contour:1:0');
  await expect(page.locator('#inspector-title')).toHaveText('Kontur 1');
  // Hele laget.
  await row(page, 'l1').click();
  await expect(viewport(page)).toHaveAttribute('data-selection', 'layer:1');
  await expect(page.locator('#inspector-title')).toHaveText('Lag 1');
  // Trykk på grenens kurve i 3D velger grenen i listen.
  await closeLayersOnMobile(page, isMobile);
  const p = await curveScreen(page, 3, 0.7);
  await (isMobile ? page.touchscreen.tap(p.x, p.y) : page.mouse.click(p.x, p.y));
  await expect(viewport(page)).toHaveAttribute('data-selection', 'spline:3');
  await (await expectRow(page, 's3')).toHaveClass(/selected/);
  // Trykk i tomt rom fjerner valget.
  const box = (await viewport(page).boundingBox())!;
  await (isMobile ? page.touchscreen.tap(box.x + box.width - 10, box.y + 10) : page.mouse.click(box.x + box.width - 10, box.y + 10));
  await expect(viewport(page)).toHaveAttribute('data-selection', '');
});

test('laget kan navngis, og navnet kan angres', async ({ page, isMobile }) => {
  test.skip(isMobile, 'Dobbeltklikk testes med mus');
  await page.goto('/');
  await newSpline(page);
  await row(page, 'l1').dblclick();
  const input = page.locator('#layer-list input.rename');
  await input.fill('Kropp');
  await input.press('Enter');
  await (await expectRow(page, 'l1')).toContainText('Kropp');
  await row(page, 'l1').click();
  await expect(page.locator('#inspector-title')).toHaveText('Kropp');
  await page.keyboard.press('Control+Z');
  await (await expectRow(page, 'l1')).toContainText('Lag 1');
});

test.describe('skjul og lås med mus', () => {
  test.skip(({ isMobile }) => isMobile, 'Drag testes med mus');

  test('skjult lag kan ikke treffes eller redigeres', async ({ page }) => {
    await page.goto('/');
    await newSpline(page);
    const tip = await anchorScreen(page, 1, -1);
    await row(page, 'l1').getByRole('button', { name: 'Skjul laget' }).click();
    await expect(viewport(page)).toHaveAttribute('data-hidden-layers', '1');
    await page.mouse.move(tip.x, tip.y);
    await page.mouse.down();
    await page.mouse.move(tip.x, tip.y - 40, { steps: 4 });
    await page.mouse.up();
    expect(await length(page, 1)).toBeCloseTo(40, 6);
    await page.mouse.click(tip.x, tip.y, { button: 'right' });
    await expect(page.locator('.context-menu')).toContainText('Arbeidsrommet');
    await page.keyboard.press('Escape');
    await row(page, 'l1').getByRole('button', { name: 'Vis laget' }).click();
    await expect(viewport(page)).toHaveAttribute('data-hidden-layers', '0');
  });

  test('låst lag vises, men kan ikke redigeres før det låses opp', async ({ page }) => {
    await page.goto('/');
    await newSpline(page);
    await row(page, 'l1').getByRole('button', { name: 'Lås laget' }).click();
    await expect(viewport(page)).toHaveAttribute('data-locked-layers', '1');
    const tip = await anchorScreen(page, 1, -1);
    await page.mouse.move(tip.x, tip.y);
    await page.mouse.down();
    await page.mouse.move(tip.x, tip.y - 40, { steps: 4 });
    await page.mouse.up();
    expect(await length(page, 1)).toBeCloseTo(40, 6);
    await row(page, 's1').click();
    await expect(page.locator('#edit-sections')).toHaveAttribute('disabled', '');
    await expect(page.getByRole('button', { name: 'Ny gren', exact: true })).toBeDisabled();
    await expect(page.locator('#locked-info')).toBeVisible();
    await page.locator('#layer-locked').uncheck();
    await expect(viewport(page)).toHaveAttribute('data-locked-layers', '0');
    // Draget på det låste laget roterte kameraet, så tuppen ligger et annet sted nå.
    const tip2 = await anchorScreen(page, 1, -1);
    await page.mouse.move(tip2.x, tip2.y);
    await page.mouse.down();
    await page.mouse.move(tip2.x, tip2.y - 40, { steps: 4 });
    await page.mouse.up();
    await expect.poll(() => length(page, 1)).toBeGreaterThan(45);
  });

  test('tuppen på forelderen kan dras når en gren sitter i tuppen', async ({ page }) => {
    await page.goto('/');
    await newSpline(page);
    // Valgt tupp: grenen festes der.
    const t0 = await anchorScreen(page, 1, -1);
    await page.mouse.click(t0.x, t0.y);
    await newBranch(page);
    await expect(viewport(page)).toHaveAttribute('data-splines', '2');
    expect(dist((await anchorsWorld(page, 2))[0], (await anchorsWorld(page, 1))[1])).toBeLessThan(1e-6);
    const child = await anchorsWorld(page, 2);
    const tip = await anchorScreen(page, 1, -1);
    await page.mouse.move(tip.x, tip.y);
    await page.mouse.down();
    await page.mouse.move(tip.x, tip.y - 40, { steps: 5 });
    await expect(status(page)).toContainText('Trekk ut');
    await page.mouse.up();
    await expect.poll(() => length(page, 1)).toBeGreaterThan(45);
    // Grenen følger tuppen og beholder sin egen form.
    const moved = await anchorsWorld(page, 2);
    expect(dist(moved[0], (await anchorsWorld(page, 1))[1])).toBeLessThan(1e-6);
    expect(dist(moved[1], moved[0])).toBeCloseTo(dist(child[1], child[0]), 6);
  });
});

test('Bézier: et trykk på kurven setter inn et punkt uten å endre formen', async ({ page, isMobile }) => {
  await page.goto('/');
  await newSpline(page);
  const before = await anchorsWorld(page, 1);
  await page.getByRole('button', { name: 'Bézier' }).click();
  const p = await curveScreen(page, 1, 0.5);
  await (isMobile ? page.touchscreen.tap(p.x, p.y) : page.mouse.click(p.x, p.y));
  await expect(status(page)).toContainText('Satte inn et ankerpunkt');
  const after = await anchorsWorld(page, 1);
  expect(after.length).toBe(before.length + 1);
  expect(dist(after[0], before[0])).toBeLessThan(1e-9);
  expect(dist(after[after.length - 1], before[before.length - 1])).toBeLessThan(1e-9);
  expect(await length(page, 1)).toBeCloseTo(40, 6);
  // Det nye punktet ligger på den gamle linjen.
  const mid = after[1];
  expect(Math.hypot(mid[0] - before[0][0], mid[2] - before[0][2])).toBeLessThan(1e-6);
  expect(mid[1] - before[0][1]).toBeGreaterThan(5);
  expect(mid[1] - before[0][1]).toBeLessThan(35);
});

test('lagre og åpne prosjektet bevarer lag, navn, grener og konturer', async ({ page, isMobile }) => {
  await page.goto('/');
  await newSpline(page);
  await row(page, 's1').click();
  await newBranch(page);
  await expect(viewport(page)).toHaveAttribute('data-splines', '2');
  await page.getByRole('button', { name: 'Ny kontur' }).click();
  await row(page, 'l1').click();
  await page.locator('#object-name').fill('Kropp');
  await page.locator('#object-name').press('Enter');
  await (await expectRow(page, 'l1')).toContainText('Kropp');
  await closeLayersOnMobile(page, isMobile);
  await newSpline(page, '+X');
  await row(page, 'l3').getByRole('button', { name: 'Lås laget' }).click();
  await expect(viewport(page)).toHaveAttribute('data-locked-layers', '1');
  const branch = await anchorsWorld(page, 2);

  const download = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Lagre' }).click();
  const file = await (await download).path();
  expect((await download).suggestedFilename()).toBe('skapning.cfproj');

  await page.reload();
  await expect(page.getByRole('button', { name: 'Åpne' })).toBeEnabled();
  await expect(viewport(page)).toHaveAttribute('data-splines', '0');
  await page.locator('#project-file').setInputFiles(file);
  await expect(status(page)).toContainText('Åpnet');
  await expect(viewport(page)).toHaveAttribute('data-splines', '3');
  await expect(viewport(page)).toHaveAttribute('data-locked-layers', '1');
  await expect(viewport(page)).toHaveAttribute('data-profiles', '1');
  await (await expectRow(page, 'l1')).toContainText('Kropp');
  const opened = await anchorsWorld(page, 2);
  opened.forEach((a, i) => expect(dist(a, branch[i])).toBeLessThan(1e-9));
  // Historikken starter fra filen.
  await expect(page.getByRole('button', { name: 'Angre' })).toBeDisabled();
});

test('langt trykk med finger åpner hurtigmenyen', async ({ page, isMobile }) => {
  test.skip(!isMobile, 'Berøring testes på mobil');
  await page.goto('/');
  await newSpline(page);
  const p = await curveScreen(page, 1, 0.5);
  const canvas = page.locator('#viewport canvas');
  const init = { pointerType: 'touch', pointerId: 7, isPrimary: true, button: 0, buttons: 1, clientX: p.x, clientY: p.y, bubbles: true };
  await canvas.dispatchEvent('pointerdown', init);
  await page.waitForTimeout(800);
  await canvas.dispatchEvent('pointerup', { ...init, buttons: 0 });
  await expect(page.locator('.context-menu')).toBeVisible();
  await expect(page.locator('.context-menu')).toContainText('Spline 1');
  // Ingen endring av dokumentet.
  expect(await length(page, 1)).toBeCloseTo(40, 6);
});
