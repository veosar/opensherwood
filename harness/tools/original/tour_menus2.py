"""Second scripted tour: the screens the first pass missed. See tour_menus.py."""
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import tour_menus as T  # noqa: E402
from tour_menus import MENU_X, ROWS, OPT_ROWS, click, key, shot, same  # noqa: E402
import rhcap  # noqa: E402


def main():
    rhcap.focus(T.H)
    img = shot("t2_start")
    ref = T.__dict__.get("ref")
    click(MENU_X, ROWS["options"], wait=1.5); options = shot("t2_options")
    for k in ("sounds", "shortcuts"):
        click(MENU_X, OPT_ROWS[k], wait=1.8); s = shot("options_" + k)
        if same(s, options):
            print("!! click on", k, "did nothing, retrying")
            click(MENU_X, OPT_ROWS[k], wait=1.8); shot("options_" + k)
        key("escape", wait=1.5); e = shot("options_%s_after_escape" % k)
        print(k, "escape returns to options:", same(e, options))
        if not same(e, options):
            click(MENU_X, OPT_ROWS["back"], wait=1.5); shot("options_%s_after_back" % k)
    click(MENU_X, OPT_ROWS["back"], wait=1.5); main_menu = shot("t2_main")
    for k in ("load", "select", "credits"):
        click(MENU_X, ROWS[k], wait=2.0); s = shot("menu_" + k)
        if k == "credits":
            time.sleep(5.0); shot("menu_credits_later")
        key("escape", wait=1.5); e = shot("menu_%s_after_escape" % k)
        print(k, "escape returns to main:", same(e, main_menu))
    shot("t2_end")


if __name__ == "__main__":
    main()
