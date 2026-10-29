import { expect, test, type Page } from '@playwright/test';
import { anchorScreen, anchorsWorld, curveScreen, newSpline, status, viewport } from './helpers';

type V = [number, number, number];
const sub = (a: V, b: V): V => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const norm = (a: V): V => {
  const l = Math.hypot(...a);
  return [a[0] / l, a[1] / l, a[2] / l];
};
const dist = (a: V, b: V) => Math.hypot(...sub(a, b));
const info = (page: Page, s: number) => page.evaluate((id) => window.creatureForgeTest!.splineInfo(id), s);
/** Retningen ut fra festet: fra første til andre punkt. */
const direction = async (page: Page, s: number) => {
  const a = await anchorsWorld(page, s);
  return norm(sub(a[1], a[0]));
};
const expectSame = (a: V[], b: V[]) => {
  expect(a.length).toBe(b.length);
  a.forEach((p, i) => expect(dist(p, b[i])).toBeLessThan(1e-9));
};

async function branchByAxis(page: Page, axis: string) {
  const before = Number(await viewport(page).getAttribute('data-splines'));
  await page.getByRole('button', { name: 'Ny gren', exact: true }).click();
  await page.locator('.axis-label', { hasText: axis }).click();
  await expect(viewport(page)).toHaveAttribute('data-splines', String(before + 1));
}

test('ny gren får sin egen retning, og forelderen endres ikke', async ({ page }) => {
  await page.goto('/');
  await newSpline(page);
  const parent = await anchorsWorld(page, 1);
  await branchByAxis(page, '+Z');
  expect(await info(page, 2)).toMatchObject({ parent: 1 });
  const d = await direction(page, 2);
  expect(d[2]).toBeCloseTo(1, 9);
  expectSame(await anchorsWorld(page, 1), parent);
  // Feste midt på forelderen.
  expect(dist((await anchorsWorld(page, 2))[0], [parent[0][0], parent[0][1] + 20, parent[0][2]])).toBeLessThan(1e-6);
});

test.describe('med mus', () => {
  test.skip(({ isMobile }) => isMobile, 'Drag med mus');

  test('retningshåndtaket gir retning og lengde, og trekking av grenen endrer ikke forelderen', async ({ page }) => {
    await page.goto('/');
    await newSpline(page);
    const parent = await anchorsWorld(page, 1);
    await page.getByRole('button', { name: 'Ny gren', exact: true }).click();
    const handle = page.locator('.axis-free');
    const box = (await handle.boundingBox())!;
    const start = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
    await page.mouse.move(start.x, start.y);
    await page.mouse.down();
    await page.mouse.move(start.x + 40, start.y - 60, { steps: 6 });
    await page.mouse.up();
    await expect(viewport(page)).toHaveAttribute('data-splines', '2');
    const a = await anchorsWorld(page, 2);
    // Dratt oppover på skjermen: grenen peker oppover (+y) og er ikke vinkelrett på forelderen.
    expect(norm(sub(a[1], a[0]))[1]).toBeGreaterThan(0.3);
    expectSame(await anchorsWorld(page, 1), parent);
    // Trekk grenens tupp videre ut: forelderen er uendret.
    const tip = await anchorScreen(page, 2, -1);
    await page.mouse.move(tip.x, tip.y);
    await page.mouse.down();
    await page.mouse.move(tip.x + 30, tip.y - 30, { steps: 5 });
    await page.mouse.up();
    await expect(status(page)).toContainText('Trekk ut');
    expectSame(await anchorsWorld(page, 1), parent);
  });

  for (const mode of ['Følg forelder', 'Behold retning'] as const) {
    test(`bøying av forelderen med «${mode}»`, async ({ page }) => {
      await page.goto('/');
      await newSpline(page);
      await branchByAxis(page, '+Z');
      if (mode === 'Behold retning') {
        const shown = await anchorsWorld(page, 2);
        await page.getByRole('button', { name: mode }).click();
        await expect.poll(async () => (await info(page, 2))?.orientation).toBe('keep_direction');
        // Byttet gir ikke noe hopp.
        expectSame(await anchorsWorld(page, 2), shown);
      }
      const before = await direction(page, 2);
      const childBefore = await anchorsWorld(page, 2);
      // Bøy forelderen med Alt + Shift på tuppen.
      const tip = await anchorScreen(page, 1, -1);
      await page.mouse.move(tip.x, tip.y);
      await page.mouse.down();
      await page.keyboard.down('Alt');
      await page.keyboard.down('Shift');
      await page.mouse.move(tip.x + 150, tip.y + 20, { steps: 8 });
      await page.mouse.up();
      await page.keyboard.up('Shift');
      await page.keyboard.up('Alt');
      await expect(status(page)).toContainText('Bøy');
      const after = await direction(page, 2);
      const childAfter = await anchorsWorld(page, 2);
      // Festet har flyttet seg med forelderen.
      expect(dist(childAfter[0], childBefore[0])).toBeGreaterThan(1);
      const turned = Math.acos(Math.min(1, before[0] * after[0] + before[1] * after[1] + before[2] * after[2]));
      if (mode === 'Behold retning') expect(turned).toBeLessThan(1e-6);
      else expect(turned).toBeGreaterThan(0.05);
      // Grenens egen lengde er uendret.
      expect(dist(childAfter[1], childAfter[0])).toBeCloseTo(dist(childBefore[1], childBefore[0]), 6);
    });
  }

  test('festet dras gjennom et kryss over på en annen gren, med undergrener, og kan angres', async ({ page }) => {
    await page.goto('/');
    await newSpline(page);
    // Gren A midt på hovedsplinen, ut langs +X.
    await branchByAxis(page, '+X');
    // Gren B høyere opp, ut langs +Z, fra hurtigmenyen på kurven.
    const p = await curveScreen(page, 1, 0.85);
    await page.mouse.click(p.x, p.y, { button: 'right' });
    await page.locator('.context-menu').getByRole('menuitem', { name: 'Ny gren her' }).click();
    await page.locator('.axis-label', { hasText: '+Z' }).click();
    await expect(viewport(page)).toHaveAttribute('data-splines', '3');
    // Undergren C på B.
    await branchByAxis(page, '+Y');
    expect(await info(page, 4)).toMatchObject({ parent: 3 });
    const lenB = dist(...((await anchorsWorld(page, 3)).slice(0, 2) as [V, V]));

    // Dra B sitt feste ned langs hovedsplinen til krysset med A, og videre ut på A.
    const from = await anchorScreen(page, 3, 0);
    const junction = await curveScreen(page, 1, 0.5);
    const onA = await curveScreen(page, 2, 0.7);
    await page.mouse.move(from.x, from.y);
    await page.mouse.down();
    await page.mouse.move(junction.x, junction.y, { steps: 10 });
    await expect(status(page)).toContainText('dra videre inn på');
    await page.mouse.move(onA.x, onA.y, { steps: 10 });
    await expect(status(page)).toContainText('Flytt festet langs Gren 2');
    await page.mouse.up();
    await expect.poll(async () => (await info(page, 3))?.parent).toBe(2);
    expect((await info(page, 3))!.at).toBeGreaterThan(0.4);
    // Undergrenen er med og sitter fortsatt på B.
    expect(await info(page, 4)).toMatchObject({ parent: 3 });
    const b = await anchorsWorld(page, 3);
    expect(dist(b[1], b[0])).toBeCloseTo(lenB, 6);
    await expect(viewport(page)).toHaveAttribute('data-splines', '4');

    // Angre setter festet tilbake på hovedsplinen.
    await page.keyboard.press('Control+Z');
    await expect.poll(async () => (await info(page, 3))?.parent).toBe(1);
  });

  test('festet kan ikke dras over på grenens egen undergren', async ({ page }) => {
    await page.goto('/');
    await newSpline(page);
    await branchByAxis(page, '+X');
    await branchByAxis(page, '+Y');
    expect(await info(page, 3)).toMatchObject({ parent: 2 });
    // Dra festet til gren 2 mot undergrenen 3.
    const from = await anchorScreen(page, 2, 0);
    const onChild = await curveScreen(page, 3, 0.6);
    await page.mouse.move(from.x, from.y);
    await page.mouse.down();
    await page.mouse.move(onChild.x, onChild.y, { steps: 12 });
    await page.mouse.up();
    await expect(viewport(page)).toHaveAttribute('data-revision', /\d+/);
    expect((await info(page, 2))!.parent).toBe(1);
    expect((await info(page, 3))!.parent).toBe(2);
  });
});
