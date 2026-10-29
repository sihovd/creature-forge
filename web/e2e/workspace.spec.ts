import { expect, test, type Page } from '@playwright/test';
import { newSpline, viewport } from './helpers';

/** En form å trekke i: en ny spline med mesh rundt. */
async function addSphere(page: Page) {
  await page.goto('/');
  await newSpline(page);
  await expect(viewport(page)).toHaveAttribute('data-shape-count', '1');
}

async function grip(page: Page) {
  const p = await page.evaluate(() => window.creatureForgeTest!.gripNearCenter());
  expect(p).not.toBeNull();
  return p!;
}

test('appen starter med et tomt arbeidsrom', async ({ page }) => {
  await page.goto('/');
  await expect(viewport(page).locator('canvas')).toBeVisible();
  await expect(page.locator('#status')).toContainText('Geometrikjerne klar');
  await expect(viewport(page)).toHaveAttribute('data-shape-count', '0');
  await expect(page.locator('#empty-hint')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Angre' })).toBeDisabled();
});

test('«Legg til kule» er fjernet, og en ny spline får mesh med kontrollpunkter', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('button', { name: 'Legg til kule' })).toHaveCount(0);
  await addSphere(page);
  await expect(page.locator('#empty-hint')).toBeHidden();
  expect(Number(await viewport(page).getAttribute('data-control-points'))).toBeGreaterThan(20);
});

test('CF02: punktavstand endrer grepene, ikke formen', async ({ page }) => {
  await addSphere(page);
  const before = Number(await viewport(page).getAttribute('data-control-points'));
  const revision = await viewport(page).getAttribute('data-revision');
  await page.locator('#spacing').fill('12');
  await page.locator('#spacing').press('Enter');
  await expect(page.locator('#status')).toContainText('Punktavstand 12 mm');
  expect(Number(await viewport(page).getAttribute('data-control-points'))).toBeLessThan(before);
  await expect(viewport(page)).toHaveAttribute('data-revision', revision!);
});

test.describe('formgrep med mus', () => {
  test.skip(({ isMobile }) => isMobile, 'Berøring testes separat');

  test('CF03: dra et punkt ut, angre og gjør om', async ({ page }) => {
    await addSphere(page);
    const p = await grip(page);
    await page.mouse.move(p.x, p.y);
    await page.mouse.down();
    await page.mouse.move(p.x, p.y - 30, { steps: 4 });
    await page.mouse.move(p.x, p.y - 60, { steps: 4 });
    await page.mouse.up();
    await expect(page.locator('#status')).toContainText(/Trakk ut|Presset inn/);
    await expect(viewport(page)).toHaveAttribute('data-revision', '2');

    await page.getByRole('button', { name: 'Angre' }).click();
    await expect(page.locator('#status')).toContainText('Angret');
    await expect(viewport(page)).toHaveAttribute('data-shape-count', '1');
    await page.keyboard.press('Control+Shift+Z');
    await expect(page.locator('#status')).toContainText('Gjort om');
    await expect(page.getByRole('button', { name: 'Gjør om' })).toBeDisabled();
  });

  test('CF06: Escape avbryter draget uten å endre dokumentet', async ({ page }) => {
    await addSphere(page);
    const p = await grip(page);
    await page.mouse.move(p.x, p.y);
    await page.mouse.down();
    await page.mouse.move(p.x, p.y - 50, { steps: 5 });
    await page.keyboard.press('Escape');
    await page.mouse.up();
    await expect(page.locator('#status')).toContainText('avbrutt');
    await expect(viewport(page)).toHaveAttribute('data-revision', '1');
    await expect(page.getByRole('button', { name: 'Angre' })).toBeEnabled();
  });

  test('prikkene trekker ut uten å lage spliner', async ({ page }) => {
    await addSphere(page);
    await expect(viewport(page)).toHaveAttribute('data-splines', '1');
    const p = await grip(page);
    await page.mouse.move(p.x, p.y);
    await page.mouse.down();
    await page.mouse.move(p.x, p.y - 50, { steps: 5 });
    await page.mouse.up();
    await expect(page.locator('#status')).toContainText(/Trakk ut|Presset inn/);
    await expect(viewport(page)).toHaveAttribute('data-revision', '2');
    await expect(viewport(page)).toHaveAttribute('data-splines', '1');
  });
});
