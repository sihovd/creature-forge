import { expect, test, type Page } from '@playwright/test';
import { newBranch, newSpline, status, viewport } from './helpers';

async function addSpline(page: Page) {
  await page.goto('/');
  await newSpline(page);
}

const anchor = async (page: Page, spline: number, index: number) => {
  const p = await page.evaluate(([s, i]) => window.creatureForgeTest!.splineAnchor(s, i), [spline, index]);
  expect(p).not.toBeNull();
  return p!;
};
const length = (page: Page, spline: number) =>
  page.evaluate((s) => window.creatureForgeTest!.splineLength(s), spline) as Promise<number>;

test('ny spline vises uten kule, og angre fjerner den', async ({ page }) => {
  await addSpline(page);
  await expect(page.locator('#empty-hint')).toBeHidden();
  expect(await length(page, 1)).toBeCloseTo(40, 3);
  await page.getByRole('button', { name: 'Angre' }).click();
  await expect(viewport(page)).toHaveAttribute('data-splines', '0');
});

test.describe('skjelett med mus', () => {
  test.skip(({ isMobile }) => isMobile, 'Berøring testes separat');

  test('dra tuppen ut forlenger splinen, og angre gjenoppretter', async ({ page }) => {
    await addSpline(page);
    const tip = await anchor(page, 1, -1);
    await page.mouse.move(tip.x, tip.y);
    await page.mouse.down();
    await page.mouse.move(tip.x, tip.y - 40, { steps: 5 });
    await expect(status(page)).toContainText('Trekk ut');
    await page.mouse.up();
    await expect(viewport(page)).toHaveAttribute('data-revision', '2');
    expect(await length(page, 1)).toBeGreaterThan(45);
    await page.keyboard.press('Control+Z');
    await expect(viewport(page)).toHaveAttribute('data-revision', '3');
    expect(await length(page, 1)).toBeCloseTo(40, 3);
  });

  test('Alt + Shift bøyer med låst lengde, og scroll endrer radien', async ({ page }) => {
    await addSpline(page);
    const tip = await anchor(page, 1, -1);
    await page.mouse.move(tip.x, tip.y);
    await page.mouse.down();
    await page.keyboard.down('Alt');
    await page.keyboard.down('Shift');
    await page.mouse.move(tip.x + 120, tip.y, { steps: 6 });
    await expect(status(page)).toContainText('Bøy');
    const before = await status(page).textContent();
    await page.mouse.wheel(0, -200);
    await expect(status(page)).not.toHaveText(before!);
    await page.mouse.up();
    await page.keyboard.up('Shift');
    await page.keyboard.up('Alt');
    await expect(viewport(page)).toHaveAttribute('data-revision', '2');
    // Buelengden er låst under bøyingen.
    expect(Math.abs((await length(page, 1)) - 40)).toBeLessThan(0.01);
  });

  test('en gren følger når forelderen panoreres med Alt', async ({ page }) => {
    await addSpline(page);
    await newBranch(page);
    await expect(viewport(page)).toHaveAttribute('data-splines', '2');
    const childBefore = await anchor(page, 2, -1);
    const tip = await anchor(page, 1, -1);
    await page.mouse.move(tip.x, tip.y);
    await page.mouse.down();
    await page.keyboard.down('Alt');
    await page.mouse.move(tip.x + 60, tip.y, { steps: 5 });
    await expect(status(page)).toContainText('Panorer');
    await page.mouse.up();
    await page.keyboard.up('Alt');
    await expect(viewport(page)).toHaveAttribute('data-revision', '3');
    const childAfter = await anchor(page, 2, -1);
    expect(Math.hypot(childAfter.x - childBefore.x, childAfter.y - childBefore.y)).toBeGreaterThan(5);
  });
});
