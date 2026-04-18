import asyncio
import os
from playwright.async_api import async_playwright

async def capture():
    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True)
        page = await browser.new_page(viewport={"width": 1920, "height": 1080})
        
        # Load the local HTML snapshot
        abs_path = os.path.abspath("tests/fixtures/hn_snapshot.html")
        await page.goto(f"file://{abs_path}")
        
        # Capture screenshot
        await page.screenshot(path="tests/fixtures/hn_chrome_truth.png")
        await browser.close()
        print("Chrome ground truth captured successfully.")

if __name__ == "__main__":
    asyncio.run(capture())
