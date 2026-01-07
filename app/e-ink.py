from playwright.sync_api import sync_playwright
import json
import png_to_bit
from png_to_bit import prepare_img
import time


def take_shot(token, base_url, ink_url, round_time):
    with sync_playwright() as p:
        print("Go..")
        browser = p.webkit.launch(headless=True)
        context = browser.new_context()
        page = context.new_page()
        print(f"Navigazione verso {base_url} per impostare il contesto...")
        page.goto(base_url)

        hass_tokens = {
            "hassUrl": base_url,
            "access_token": token,
            "token_type": "Bearer",
        }

        print("Iniezione del token nel localStorage...")
        page.evaluate(
            "(tokensStr) => { localStorage.setItem('hassTokens', tokensStr); }",
            json.dumps(hass_tokens),
        )

        try:
            print(f"Navigazione verso {ink_url}...")
            while True:
                page.set_viewport_size({"width": 400, "height": 350})
                page.goto(ink_url)
                page.wait_for_load_state("networkidle")
                page.wait_for_timeout(2000)

                # clip={"x": 390, "y": 5, "width": 500, "height": 320}
                buffer = page.screenshot()
                img = png_to_bit.prepare_img(buffer)
                b = png_to_bit.image_to_bit_buffer(img, out_bw_name="uno.png")
                png_to_bit.save_bin(b)

                print("Done")
                time.sleep(round_time)
        except KeyboardInterrupt as e:
            print("Close Browser")
            browser.close()


if __name__ == "__main__":
    import os

    token = os.getenv("TOKEN", None)
    ink_url = os.getenv("INK_URL", None)
    base_url = os.getenv("BASE_URL", None)
    time_round = os.getenv("TIME_ROUND", 5)
    if token is None or ink_url is None or base_url is None:
        print("Invalid parameters")
        print(f"{token}")
        print(f"{ink_url}")
        print(f"{base_url}")
        sys.exit(1)

    take_shot(token, base_url, ink_url, time_round)
