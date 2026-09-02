"""Scripted tour of the original game's menus (analyst helper, no game bytes).

Runs against a game already showing its main menu. Every step: input, wait, screenshot.
Falls back to a 'Back' button click when Escape does not leave a sub-screen.
"""
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import pyautogui  # noqa: E402
import rhcap  # noqa: E402
from PIL import ImageChops  # noqa: E402

MENU_X = 748
ROWS = {"play": 358, "load": 399, "select": 440, "options": 481, "movies": 522, "credits": 563, "exit": 604}
OPT_ROWS = {"graphics": 481, "sounds": 522, "shortcuts": 563, "back": 604}
H, _ = rhcap.find_window()


def same(a, b, tol=0.02):
    d = ImageChops.difference(a.convert("L"), b.convert("L")).point(lambda v: 255 if v > 24 else 0)
    n = sum(1 for v in d.getdata() if v)
    return n < tol * a.size[0] * a.size[1]


def shot(name):
    img = rhcap.grab(H)
    img.save(os.path.join(rhcap.CAPDIR, name + ".png"))
    print("shot", name)
    return img


def click(x, y, wait=1.2):
    rhcap.rclick(H, x, y)
    time.sleep(wait)


def key(k, wait=1.0):
    if rhcap.focus(H):
        pyautogui.press(k)
    time.sleep(wait)


def back_to(reference, name, back_xy=(MENU_X, OPT_ROWS["back"])):
    """Try Escape, then the Back button, until the screen matches `reference`."""
    key("escape")
    img = shot(name + "_after_escape")
    if same(img, reference):
        print(name, ": escape works")
        return True
    click(*back_xy)
    img = shot(name + "_after_back")
    if same(img, reference):
        print(name, ": back button works, escape does not")
        return True
    print(name, ": STILL NOT BACK")
    return False


def main():
    rhcap.focus(H)
    main_menu = shot("menu_main")
    rhcap.rmove(H, MENU_X, ROWS["play"]); time.sleep(0.5); shot("menu_hover_play")
    click(MENU_X, ROWS["options"]); options = shot("options_main")
    for k in ("graphics", "sounds", "shortcuts"):
        click(MENU_X, OPT_ROWS[k]); shot("options_" + k)
        back_to(options, "options_" + k)
    click(MENU_X, OPT_ROWS["back"]); shot("menu_after_options_back")
    for k in ("load", "select", "credits", "movies"):
        click(MENU_X, ROWS[k], wait=2.0); shot("menu_" + k)
        if k in ("credits", "movies"):
            time.sleep(4.0); shot("menu_" + k + "_later")
        back_to(main_menu, "menu_" + k)
    shot("menu_end")


if __name__ == "__main__":
    main()
