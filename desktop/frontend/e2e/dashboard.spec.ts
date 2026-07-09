import { test, expect } from '@playwright/test'

test.describe('praxis Dashboard E2E', () => {

  test('login page renders correctly', async ({ page }) => {
    await page.goto('/')
    // Logo and title
    await expect(page.locator('h1:has-text("praxis")')).toBeVisible()
    await expect(page.locator('text=Neural Command Center')).toBeVisible()
    // Token input
    await expect(page.locator('input[type="password"]')).toBeVisible()
    // Access button
    await expect(page.locator('button:has-text("Access System")')).toBeVisible()
  })

  test('login button disabled without token', async ({ page }) => {
    await page.goto('/')
    const btn = page.locator('button:has-text("Access System")')
    await expect(btn).toBeDisabled()
  })

  test('login button enabled with token input', async ({ page }) => {
    await page.goto('/')
    await page.locator('input[type="password"]').fill('test-token-123')
    const btn = page.locator('button:has-text("Access System")')
    await expect(btn).toBeEnabled()
  })

  test('can type into token field', async ({ page }) => {
    await page.goto('/')
    const input = page.locator('input[type="password"]')
    await input.fill('my-jwt-token')
    await expect(input).toHaveValue('my-jwt-token')
  })

  test('login form submits and shows loading state', async ({ page }) => {
    await page.goto('/')
    await page.locator('input[type="password"]').fill('invalid-token')
    await page.locator('button:has-text("Access System")').click()
    // Should show loading state (either "Authenticating..." or error)
    // Since no backend, it will eventually show an error or loading spinner
    await expect(page.locator('button:has-text("Authenticating")')).toBeVisible({ timeout: 2000 })
  })

  test('footer shows security note', async ({ page }) => {
    await page.goto('/')
    await expect(page.locator('text=Token stored locally')).toBeVisible()
  })

  test('page has correct title', async ({ page }) => {
    await page.goto('/')
    await expect(page).toHaveTitle(/praxis/i)
  })
})
