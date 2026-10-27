import { expect, test } from '@playwright/test';

test('appen starter med et tomt arbeidsrom', async ({ page }) => {
  await page.goto('/');
  const viewport = page.locator('#viewport');
  await expect(viewport.locator('canvas')).toBeVisible();
  await expect(page.locator('#status')).toContainText('Geometrikjerne klar');
  await expect(viewport).toHaveAttribute('data-shape-count', '0');
  await expect(page.locator('#empty-hint')).toBeVisible();
});

test('«Legg til kule» lager en kule i Rust-kjernen', async ({ page }) => {
  await page.goto('/');
  const button = page.getByRole('button', { name: 'Legg til kule' });
  await expect(button).toBeEnabled();
  await button.click();
  await expect(page.locator('#viewport')).toHaveAttribute('data-shape-count', '1');
  await expect(page.locator('#empty-hint')).toBeHidden();
  await expect(page.locator('#status')).toContainText('trekanter');
});
