import { expect, test } from '@playwright/test';

// Telefoner støttes ikke (for liten skjerm). PC og iPad Pro får editoren.
test('telefoner får en melding om for liten skjerm, større skjermer får editoren', async ({ page }, info) => {
  await page.goto('/');
  const message = page.getByRole('alert').filter({ hasText: 'Skjermen er for liten' });
  if (info.project.name === 'telefon') {
    await expect(message).toBeVisible();
    await expect(page.getByRole('button', { name: 'Ny spline' })).toBeHidden();
    // Liggende telefon er også for liten.
    await page.setViewportSize({ width: 915, height: 412 });
    await expect(message).toBeVisible();
  } else {
    await expect(message).toBeHidden();
    await expect(page.getByRole('button', { name: 'Ny spline' })).toBeVisible();
  }
});
