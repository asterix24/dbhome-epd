from playwright.sync_api import sync_playwright
import json
import png_to_bit
from png_to_bit import prepare_img
import time


def take_shot(token, base_url, ink_url, round_time):
    with sync_playwright() as p:
        print("Shot go..")
        browser = p.webkit.launch(headless=True)
        context = browser.new_context()
        page = context.new_page()
        print(f"Shot open base url:{base_url}")
        page.goto(base_url)

        hass_tokens = {
            "hassUrl": base_url,
            "access_token": token,
            "token_type": "Bearer",
        }

        print("Shot set token to localStorage...")
        page.evaluate(
            "(tokensStr) => { localStorage.setItem('hassTokens', tokensStr); }",
            json.dumps(hass_tokens),
        )

        try:
            print(f"Shot open ink  {ink_url}...")
            while True:
                try:
                    page.set_viewport_size({"width": 400, "height": 350})
                    page.goto(ink_url)
                    page.wait_for_load_state("networkidle")
                    page.wait_for_timeout(2000)
                except playwright._impl._errors.Error as e:
                    print(e)
                    time.sleep(5)
                    continue

                # clip={"x": 390, "y": 5, "width": 500, "height": 320}
                buffer = page.screenshot()
                img = png_to_bit.prepare_img(buffer)
                b = png_to_bit.image_to_bit_buffer(img, out_bw_name="uno.png")
                png_to_bit.save_bin(b)

                print("Shot done")
                time.sleep(round_time)
        except KeyboardInterrupt as e:
            print("Shot Close Browser")
            browser.close()


if __name__ == "__main__":
    import os

    token = os.getenv("TOKEN", None)
    ink_url = os.getenv("INK_URL", None)
    base_url = os.getenv("BASE_URL", None)
    time_round = os.getenv("TIME_ROUND", 60)
    if token is None or ink_url is None or base_url is None:
        print("Invalid parameters")
        print(f"{token}")
        print(f"{ink_url}")
        print(f"{base_url}")
        sys.exit(1)

    take_shot(token, base_url, ink_url, time_round)
