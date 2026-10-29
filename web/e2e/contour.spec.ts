import { expect, test, type Page } from '@playwright/test';
import { newSpline, status, viewport } from './helpers';

const editor = (page: Page) => page.locator('#profile-editor svg');

const addSpline = (page: Page, axis = '+Y') => newSpline(page, axis);

const screenPoint = async (page: Page, spline: number, fraction: number) => {
  const p = await page.evaluate(([s, f]) => window.creatureForgeTest!.splineCurvePoint(s, f), [spline, fraction]);
  expect(p).not.toBeNull();
  return p!;
};
const anchors = (page: Page, spline: number) =>
  page.evaluate((s) => window.creatureForgeTest!.splineAnchorsWorld(s), spline) as Promise<[number, number, number][]>;

test('kontur: ny kontur, H-form, kopier og lim inn, og master', async ({ page }) => {
  await page.goto('/');
  await addSpline(page);
  await expect(viewport(page)).toHaveAttribute('data-splines', '1');
  const add = page.getByRole('button', { name: 'Ny kontur' });
  await expect(add).toBeEnabled();
  await add.click();
  await expect(viewport(page)).toHaveAttribute('data-profiles', '1');
  // Ny kontur er en sirkel med fire punkter.
  await expect(editor(page)).toHaveAttribute('data-points', '4');

  await page.locator('#profile-preset').selectOption('H');
  await expect(status(page)).toContainText('Kontur: H');
  await expect(editor(page)).toHaveAttribute('data-points', '12');

  // Gjør til master, kopier og lim inn: den nye er koblet til samme master.
  await page.getByRole('button', { name: 'Gjør til master' }).click();
  await expect(page.locator('#master-info')).toContainText('M1');
  await page.getByRole('button', { name: 'Kopier' }).click();
  await page.locator('#station-at').fill('90');
  await page.locator('#station-at').press('Enter');
  await expect(page.locator('#station-select')).toContainText('90 %');
  await page.getByRole('button', { name: 'Lim inn' }).click();
  await expect(viewport(page)).toHaveAttribute('data-profiles', '2');
  await expect(page.locator('#master-info')).toContainText('2 konturer');

  // En koblet kontur redigeres ikke uten å velge «Rediger master».
  await page.getByRole('button', { name: 'Gjør individuell' }).click();
  await expect(page.locator('#master-info')).toContainText('individuell');
  await page.getByRole('button', { name: 'Angre' }).click();
  await expect(page.locator('#master-info')).toContainText('M1');
});

test('konturfeltet: dobbeltklikk legger inn et punkt, og draget flytter det', async ({ page, isMobile }) => {
  test.skip(isMobile, 'Konturfeltet testes med mus');
  await page.goto('/');
  await addSpline(page);
  await page.getByRole('button', { name: 'Ny kontur' }).click();
  await expect(editor(page)).toHaveAttribute('data-points', '4');
  const box = (await editor(page).boundingBox())!;
  // Sirkelen har radius 1 av 1,6 i feltet: punktet øverst til høyre på omrisset.
  const r = (box.width / 2) * (1 / 1.6);
  const cx = box.x + box.width / 2;
  const cy = box.y + box.height / 2;
  await page.mouse.dblclick(cx + r * Math.SQRT1_2, cy - r * Math.SQRT1_2);
  await expect(editor(page)).toHaveAttribute('data-points', '5');
  const revision = Number(await viewport(page).getAttribute('data-revision'));
  // Dra det nye punktet utover.
  await page.mouse.move(cx + r * Math.SQRT1_2, cy - r * Math.SQRT1_2);
  await page.mouse.down();
  await page.mouse.move(cx + r * 1.2, cy - r * 1.2, { steps: 5 });
  await page.mouse.up();
  await expect(viewport(page)).toHaveAttribute('data-revision', String(revision + 1));
  await expect(status(page)).toContainText('Flyttet konturpunkt');
});

test.describe('med mus', () => {
  test.skip(({ isMobile }) => isMobile, 'Høyreklikk og drag testes med mus');

  test('høyreklikk gir hurtigvalg i arbeidsrommet og på formen', async ({ page }) => {
    await page.goto('/');
    await expect(page.getByRole('button', { name: 'Ny spline' })).toBeEnabled();
    const box = (await viewport(page).boundingBox())!;
    await page.mouse.click(box.x + box.width * 0.5, box.y + box.height * 0.8, { button: 'right' });
    const menu = page.locator('.context-menu');
    await expect(menu).toBeVisible();
    await expect(menu.getByRole('menuitem', { name: 'Legg til kule her' })).toHaveCount(0);
    await menu.getByRole('menuitem', { name: 'Ny spline her' }).click();
    await expect(menu).toBeHidden();
    await page.locator('.axis-label', { hasText: '+Y' }).click();
    await expect(viewport(page)).toHaveAttribute('data-shape-count', '1');
    await page.locator('#thickness').fill('20');
    await page.locator('#thickness').press('Enter');
    await expect(status(page)).toContainText('Tykkelse 20 mm');

    // Høyreklikk på formen, utenfor splinen i midten.
    const mid = await screenPoint(page, 1, 0.5);
    await page.mouse.click(mid.x + 25, mid.y, { button: 'right' });
    await expect(menu).toContainText('Lag 1');
    await menu.getByRole('menuitem', { name: 'Ny gren ut herfra' }).click();
    await expect(viewport(page)).toHaveAttribute('data-splines', '2');
    await expect(viewport(page)).toHaveAttribute('data-shape-count', '1');
  });

  test('grip kurven for å flytte hele formen', async ({ page }) => {
    await page.goto('/');
    await addSpline(page);
    const before = await anchors(page, 1);
    const mid = await screenPoint(page, 1, 0.5);
    await page.mouse.move(mid.x, mid.y);
    await page.mouse.down();
    await page.mouse.move(mid.x + 60, mid.y, { steps: 5 });
    await expect(status(page)).toContainText('Flytt hele formen');
    await page.mouse.up();
    await expect(viewport(page)).toHaveAttribute('data-revision', '2');
    const after = await anchors(page, 1);
    // Alle punkter er flyttet like mye.
    const d = after[0].map((v, i) => v - before[0][i]);
    expect(Math.hypot(...d)).toBeGreaterThan(5);
    after.forEach((a, k) => a.forEach((v, i) => expect(v - before[k][i]).toBeCloseTo(d[i], 6)));
  });

  test('enden snapper til en annen spline', async ({ page }) => {
    await page.goto('/');
    await addSpline(page);
    await addSpline(page, '+X');
    await expect(viewport(page)).toHaveAttribute('data-splines', '2');
    const target = await anchors(page, 1);
    const tip = await screenPoint(page, 2, 1);
    // Panorer tuppen på spline 2 bort til tuppen av spline 1.
    const goal = await screenPoint(page, 1, 1);
    await page.mouse.move(tip.x, tip.y);
    await page.mouse.down();
    await page.keyboard.down('Alt');
    await page.mouse.move(goal.x + 3, goal.y + 2, { steps: 8 });
    await expect(status(page)).toContainText('snapper til spline 1');
    await page.mouse.up();
    await page.keyboard.up('Alt');
    const moved = await anchors(page, 2);
    const end = moved[moved.length - 1];
    // Enden ligger på spline 1 (en loddrett linje fra target[0] til target[1]).
    expect(end[0]).toBeCloseTo(target[0][0], 3);
    expect(end[2]).toBeCloseTo(target[0][2], 3);
    expect(end[1]).toBeGreaterThan(target[0][1] - 1e-6);
    expect(end[1]).toBeLessThan(target[1][1] + 1e-6);
  });
});
