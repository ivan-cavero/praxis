import { test, expect } from '@playwright/test'

test.describe('praxis Dashboard E2E', () => {

  test('shows login page', async ({ page }) => {
    await page.goto('/')
    // Login page should have the logo and access token input
    await expect(page.locator('h1:has-text("praxis")')).toBeVisible()
    await expect(page.locator('input[type="password"]')).toBeVisible()
  })

  test('skip auth navigates to dashboard', async ({ page }) => {
    await page.goto('/')
    // Click skip authentication
    const skipBtn = page.getByText('Skip authentication')
    await skipBtn.click()

    // Dashboard should show - look for sidebar nav
    await expect(page.locator('aside')).toBeVisible()
    // Look for the sector header
    await expect(page.locator('text=Sector')).toBeVisible()
  })

  test('sidebar has all nav items after login', async ({ page }) => {
    await page.goto('/')
    await page.getByText('Skip authentication').click()

    // Wait for dashboard to render
    await expect(page.locator('aside')).toBeVisible()

    // Check nav items exist in sidebar
    const sidebar = page.locator('aside')
    await expect(sidebar.getByText('Overview')).toBeVisible()
    await expect(sidebar.getByText('Sessions')).toBeVisible()
    await expect(sidebar.getByText('Agents')).toBeVisible()
    await expect(sidebar.getByText('Config')).toBeVisible()
  })

  test('overview page shows metric cards', async ({ page }) => {
    await page.goto('/')
    await page.getByText('Skip authentication').click()
    await expect(page.locator('text=System Status')).toBeVisible()
    await expect(page.locator('text=Active Sessions')).toBeVisible()
  })

  test('agents page shows roles', async ({ page }) => {
    await page.goto('/')
    await page.getByText('Skip authentication').click()
    // Navigate to agents
    await page.locator('aside').getByText('Agents').click()
    await expect(page.locator('text=Architect')).toBeVisible()
    await expect(page.locator('text=Coder')).toBeVisible()
    await expect(page.locator('text=Security')).toBeVisible()
  })

  test('config page shows providers', async ({ page }) => {
    await page.goto('/')
    await page.getByText('Skip authentication').click()
    await expect(page.locator('aside')).toBeVisible()
    await page.locator('aside').getByText('Config').click()
    await page.waitForTimeout(500)
    await expect(page.locator('text=BACKEND API').first()).toBeVisible()
  })

  test('can logout and return to login', async ({ page }) => {
    await page.goto('/')
    await page.getByText('Skip authentication').click()
    await expect(page.locator('aside')).toBeVisible()

    // Logout
    await page.locator('aside').getByText('Disconnect').click()

    // Should return to login
    await expect(page.locator('input[type="password"]')).toBeVisible()
  })
})
