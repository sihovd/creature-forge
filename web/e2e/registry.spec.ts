import { expect, test, type Page } from '@playwright/test';
import { newSpline, status, viewport } from './helpers';

const tile = (page: Page, key: string) => page.locator(`#registry [data-key="${key}"] > button`).first();
const editorPoints = (page: Page) => page.locator('#profile-editor svg');
const row = (page: Page, key: string) => page.locator(`#layer-list [data-key="${key}"] > .row`);

test.describe('med mus', () => {
  test.skip(({ isMobile }) => isMobile, 'Panelene testes på desktop');

  test('presets kopieres inn som konturer uten kobling', async ({ page }) => {
    await page.goto('/');
    await newSpline(page);
    // Bare splinen valgt: et profilpreset gir en ny kontur.
    await tile(page, 'p:Egg').click();
    await expect(viewport(page)).toHaveAttribute('data-profiles', '1');
    await expect(status(page)).toContainText('Ny kontur: Egg');
    await expect(editorPoints(page)).toHaveAttribute('data-points', '4');
    // Med konturen valgt erstatter et preset omrisset.
    await tile(page, 'p:Trekant').click();
    await expect(status(page)).toContainText('Kontur: Trekant (kopi, ingen kobling)');
    await expect(editorPoints(page)).toHaveAttribute('data-points', '3');
    await expect(page.locator('#master-info')).toContainText('individuell');
  });

  test('volumpreset «Hode» legger flere konturer langs splinen', async ({ page }) => {
    await page.goto('/');
    await newSpline(page);
    await tile(page, 'v:Hode').click();
    await expect(status(page)).toContainText('Volum: Hode · 5 konturer');
    await expect(viewport(page)).toHaveAttribute('data-profiles', '5');
    await row(page, 's1').click();
    await tile(page, 'v:Kule').click();
    await expect(viewport(page)).toHaveAttribute('data-profiles', '5');
    await page.keyboard.press('Control+Z');
    await expect(status(page)).toContainText('Angret');
  });

  test('egne profiler og volumer lagres i registeret og består etter lagring og åpning', async ({ page }) => {
    await page.goto('/');
    await newSpline(page);
    await tile(page, 'p:H').click();
    await page.locator('#registry-name').fill('Min H');
    await page.getByRole('button', { name: 'Lagre kontur' }).click();
    await expect(page.locator('#registry')).toContainText('Min H');
    await row(page, 's1').click();
    await tile(page, 'v:Hode').click();
    await row(page, 's1').click();
    await page.locator('#registry-name').fill('Mitt hode');
    await page.getByRole('button', { name: 'Lagre volum' }).click();
    await expect(page.locator('#registry')).toContainText('Mitt hode');

    const download = page.waitForEvent('download');
    await page.getByRole('button', { name: 'Lagre', exact: true }).click();
    const file = await (await download).path();
    await page.reload();
    await expect(page.getByRole('button', { name: 'Åpne' })).toBeEnabled();
    await page.locator('#project-file').setInputFiles(file);
    await expect(status(page)).toContainText('Åpnet');
    await expect(page.locator('#registry')).toContainText('Min H');
    await expect(page.locator('#registry')).toContainText('Mitt hode');

    // Bruk den lagrede profilen på en ny spline.
    await newSpline(page, '+X');
    await page.locator('#registry .tile', { hasText: 'Min H' }).locator('button').first().click();
    await expect(editorPoints(page)).toHaveAttribute('data-points', '12');
    // Slett en oppføring.
    await page.locator('#registry .tile', { hasText: 'Mitt hode' }).locator('.tile-remove').click();
    await expect(page.locator('#registry')).not.toContainText('Mitt hode');
  });

  test('master: redigering på modellen viser at flere konturer endres, og «Gjør individuell» bryter koblingen', async ({ page }) => {
    await page.goto('/');
    await newSpline(page);
    await page.locator('#thickness').fill('15');
    await page.locator('#thickness').press('Enter');
    await page.getByRole('button', { name: 'Ny kontur' }).click();
    await page.getByRole('button', { name: 'Gjør til master' }).click();
    await expect(page.locator('#master-info')).toContainText('M1');
    await page.getByRole('button', { name: 'Kopier' }).click();
    await page.locator('#station-at').fill('85');
    await page.locator('#station-at').press('Enter');
    await expect(page.locator('#station-select')).toContainText('85 %');
    await page.getByRole('button', { name: 'Lim inn' }).click();
    await expect(page.locator('#master-info')).toContainText('2 konturer');
    await page.getByRole('button', { name: 'Rediger kontur på modellen' }).click();
    await expect(page.locator('.contour-bar')).toContainText('endrer masteren M1 og 2 konturer');
    await page.locator('.contour-bar').getByRole('button', { name: 'Gjør individuell' }).click();
    await expect(page.locator('.contour-bar')).not.toContainText('endrer masteren');
    await page.locator('.contour-bar').getByRole('button', { name: 'Ferdig' }).click();
    await expect(page.locator('#master-info')).toContainText('individuell');
  });
});
