"""Characterization of probe reports using invented data and mocked file access."""

import contextlib
import hashlib
import importlib
import io
import json
import os
import struct
import sys
import unittest
from pathlib import Path
from typing import ClassVar
from unittest.mock import Mock, mock_open, patch

import numpy as np

PROBES = Path(__file__).resolve().parents[2] / "tools/probe"
sys.path.insert(0, str(PROBES))


def tool(name):
    # Some historical probes require the variable at import, without reading it.
    with patch.dict(os.environ, {"OPENSHERWOOD_GAME_DIR": "synthetic-only"}):
        return importlib.import_module(name)


def quad(op, a=0, b=0, c=0):
    return struct.pack("<BHHI", op, a, b, c)


def script_class(name="StartUp"):
    instructions = [
        (3, 2, 3, 0), (0x13, 0xC000, 0, 7), (0xB, 0xC000, 0, 0),
        (0xC, 3, 0, 0), (0xD, 0xC001, 0, 0), (0x11, 0x4000, 0xC001, 0),
        (8, 0xC002, 0, 0), (0x29, 0xC003, 0xC002, 0xC000),
        (0xF, 0xC003, 0, 11), (0xE, 6, 0, 0), (0xE, 12, 0, 0),
        (2, 0xC001, 0, 0), (5, 19, 0, 0), (0xA, 0xC004, 0, 0),
        (0x14, 0xC005, 0, 0x3FC00000), (0x15, 0xC006, 0xC005, 0),
        (7, 0xC004, 0, 0), (6, 0, 0, 0), (4, 0, 0, 0),
        (3, 1, 1, 0), (0x13, 0xC000, 0, 0xFFFFFFFF), (0xB, 0x4000, 0, 0),
        (0xB, 0xC000, 0, 0), (0xC, 43, 0, 0), (0xC, 99, 0, 0),
        (0x11, 0xC001, 0x8000, 0), (0x12, 0x4000, 0xC001, 0),
        (0x13, 0x4000, 0, 4), (0x14, 0x4000, 0, 0x40000000),
        (0x19, 0x4000, 0xC000, 0xC001), (0xFE, 0, 0, 0), (1, 0, 0, 0),
        (0xF, 0xC000, 0, 19), (7, 0xC000, 0, 0),
    ]
    return {"name": name, "path": "synthetic", "nvars": 1, "sizevars": 4,
            "vars": [(2, "", "counter", 0)],
            "funcs": [("ProcessMessage", 0, 1, 2, 2, 3, 0),
                      ("Initialize", 19, 1, 2, 1, 1, 0)],
            "nquads": len(instructions), "quads": b"".join(quad(*q) for q in instructions)}


def mission():
    actor = {"x": 10, "y": 20, "name": "unit1", "profile": 0, "dir": 2,
             "B": 1, "flags10": "000100", "flags": 3, "trailer": 0}
    return {
        "BOYZ": {"data": [{"tag": tag, "records": [dict(actor)]}
                          for tag in ("SCOT", "BORG", "OILE", "TOTO", "BOOM")]},
        "RAIL": {"data": [[], [{"x": 1, "y": 2, "name": "p", "program": None}],
                          [{"x": 3, "y": 4, "name": "", "program": {"tables": [
                              {"id": 4, "blocks": [(50, "", [(1, "ab"), (2, "abcdefghij")])]}]}},
                           {"x": 5, "y": 6, "program": None}]]},
        "GULP": {"data": {"polygons": [{"points": [], "name": ""},
                                       {"points": [[1, 2], [3, 4]], "name": "area"}],
                          "points": [{"x": 2, "y": 3}]}},
        "TING": {"data": [{"x": 1, "y": 2, "poly": [0, [[1, 2], [3, 4]]],
                          "u16a": 1, "u32b": 2, "u16c": 3, "u32d": 4, "i16e": -1,
                          "woaw": {"count": 2}}]},
        "ZORG": {"data": [{"x": 2, "y": 3, "a": 1, "b": 2, "flags": 3}]},
        "SKRO": {"data": [{"x": 3, "y": 4, "name": "scroll", "flags5": "00", "flags": 4}]},
        "CAVE": {"data": [{"flag": 1, "ids": [2, 3]}]},
        "HIRN": {"data": {t: {"records": [{"x": 7, "y": 8}]} for t in ("HOLE", "POW ", "BUSH")}},
        "FOOT": {"data": {"variant": 1, "map": "synthetic", "mission_id": 42}},
    }


def stable(value):
    if isinstance(value, str):
        return value.replace(chr(92), "/")
    if isinstance(value, Mock):
        return "<mock>"
    if isinstance(value, bytes):
        return value.hex()
    if isinstance(value, np.ndarray):
        return value.tolist()
    if isinstance(value, dict):
        return {str(k): stable(v) for k, v in value.items()}
    if isinstance(value, (tuple, list)):
        return [stable(v) for v in value]
    if isinstance(value, set):
        return sorted(value)
    return value


class ProbeTests(unittest.TestCase):
    # SHA-256 snapshots were recorded on the unmodified tools. They cover the
    # full text, return values, exception type/message and ordered mock calls.
    SNAPSHOTS: ClassVar[dict[str, str]] = {
        "mmove": "9718bad26edc959a30bffd2f172d09b9bd14b8e54c2387a9224c015bf2895c55",
        "capture0": "ff84c8b1772cd2fa66bbcdf18b3fcbd3ee7dc486b290c785dac911758357b719",
        "capture1": "1a19f3518d243c93b6ae4de37936a8319a584e89c854e98179a42cb77d25f38a",
        "capture2": "7126cc3b37e4bef214cd560f9af4e24673aa0e549bed36e165da18cfea15789a",
        "capture3": "4316ff00853232926b111f1e82e333c9292e9fc2d178122f56a1d429eca928ed",
        "capture4": "7c556389e94b84889222aa28f7b410b292e7e2e3102b3a77affa7ce66bfacf99",
        "capture5": "267fa37dbf0988cddc58c23d346548512a4fed9ada3c59e49d5e90ca7e58762c",
        "capture6": "ebcef4df091d308ab5f19ef67f810d39608baf45a8eafcd8cb70da254663cd20",
        "capture7": "95d9af9e1456059b18ed56e27840fd57a83be01863619b7d08a277cb3e47681b",
        "capture8": "d8355aefb16522d398e5d5737810646cfb9a22d15dffd3db8418230520c2396c",
        "capture9": "8f285abdf9e95cfc759c140b345294227cf8091f5a87d86040314836beac669a",
        "capture10": "b7d71275dcd24b3dbfbfd678373c589378ad6ac6e8bfd6bf191cf794f448d86e",
        "capture11": "366db9cd5011efe89a48f7b63f76e16c7c775cf16b1bfe67df72b55451d56a13",
        "capture12": "8ecdd8a324689bbab770a1503b0af1be5dc446cf05d43402637b8ae7a8714efb",
        "capture13": "359c32e3b3b7ab9f7839eb556ea20a497f821ce2581b1bc34c306978ba030b25",
        "capture14": "9ca56e08e5e74dd65760077c9b85953bb50eda8ff1219ea1231d429b9528464b",
        "capture15": "4a293fb51e3fbc686ad74c9c0821ff2463ddfc1cd16cc015e54fed4f6a7d6af7",
        "capture16": "4c9d231b9a5551b33e8398030b10db9d714de99cba6fb3e616a3e155ed43fe45",
        "capture17": "7a45cb9ac28c705aa0e1576deae0cd7731e673ace9d1ae5b4e3873a6159776c0",
        "capture18": "794a4b672ef0a85d46ee9470c406cf9449c5654d39916124ec5aed874afc665a",
        "capture19": "c78bf6f24f0a393181a6c5153d8d4d1eb04bc3a8ca1256b18ce327480ade4b68",
        "mmove_zero": "405c66307fb534f91ab65011a4e0480d83ee42b4ec1c78cc694131ff4397e65a",
        "frames0": "0e738923eb766a24562046480985695edcbc875ed80123ca921406fd459bd6b2",
        "frames1": "a3379f5861f0ca4b1e67d3e59b0d76536013b9611746a56fb9c445190009d522",
        "frames2": "7cecad4acbdd4f6af8d96106abed071f60f835e2d75665a9c8d10cec96e6da25",
        "frames3": "66db43de33415526048b699ec1f28f129edfae75c09a2f176b83b9c0659cd826",
        "frames4": "d311bb988e28c249749b6b49dcfbe05497b602ca1446c839ed03c163feea41a3",
        "frames5": "1b127211f27785204944c6246f23466f295431422318037da16c94325a8c1d0c",
        "frames6": "2c54788b466de313ca820e636134db0ce3d5df20b0c7d19aa05acff7f8088f63",
        "frames7": "84a68f11f6bc72a518814ae6b601966786c1da8d869c3c6a4dd131ab70740b00",
        "frames8": "378a902ed05953ba265409b48413f74886f1b1503dc2e3c5acb130d06af99617",
        "frames9": "e6108531bdbd490eafc891bf117c37f9fa0063bea32e0b4e4fd33059d27e27d9",
        "oracle0": "bba38d5185fa0a736d2820006cec7392024102ed8a09ddd95cb904fdb691c44f",
        "oracle1": "a2eaea6036aac3b4642f245de7d8689e862ea602e859a553b2a1f558993d0b31",
        "oracle2": "c7be890e8af125351aa8d3411cea1e6ca5b462a2988313825f9edcac55bce793",
        "oracle3": "b8d38293f9bba1ce7635c460b87f5dd302b56f3d2b4bc8aca569d439cbf37998",
        "oracle4": "20cc329f8fe75e99dcc4479f9eacf025ff8895c740b0aae30cac9f6cced0e769",
        "oracle5": "d25e9e9447319631b1e02340ef48195767a04325332d65506165149acec9d297",
        "oracle6": "ea9f2a8b61a68f0a8ab64ee80b011ca452fd097a656da3f938b0cc3dabad0264",
        "oracle7": "31863a748364a2ed9448eff8dc4a337a0c6277e74e5118be464074de30b53255",
        "oracle8": "8fa7be4737e1f43116add7454f23783d6925b1cea0ef2c04156c1d59aee9bab8",
        "anim": "120bf393ce7da4c0338e3189a519d9c20791f1a17d39fddb660ad6879302e81e",
        "anim--strip": "64c2ded81f9e3ea976f731f7c42a5d7e8c8631607c7a1e18953b5fe09854e307",
        "anim--blocks": "c31d02c77f5929602dcce5139f48eccb06a5389f161083c89335a8c5ba290d75",
        "chunks": "9135d0b366d3ba85fd36c7a62c5f8f11fe6dab99ec834902cfb90e15b9843411",
        "cpf_probe": "bdc8c7dfc53026337cd632a8a70a4f07ece49838043095bca1cdf0254e99788f",
        "cpf_stats": "b0e485304f80cf55bc8b06252078429c39b29ea7fbe82e3f17bc7a0d359bc2a3",
        "cpf_missing_rhs": "b6e8dbb7288dc6caf0acb00a59f97ddbf81a268cb6049b38f869e667b1c86849",
        "elements_main": "82172e1427d3f94f302f3ddf687cc8e9d7a1ccb88de0d7c3210b66555d55d4e2",
        "flow": "482a373584a709e532208ade89f9fd695a742ba252c2e5972f5d22c0dd836f91",
        "pseudo": "0aec246dd42ca4fa5d9394e0403bd5bd1fd68b8b8db273889521c3e238913fc7",
        "flow_without_callbacks": "42388179cfb1f176b38818be0e0a34524699e1bbd6a89239ab994f158c37c663",
        "flow_truncated": "af1e94e413fa0adc5a1c7dfaad71b94a2631d915392797ec91f5fac2dc7695ec",
        "load_natives": "4a0ade97e61777220d2082e4860e28adef052b475b35bca72b592e774fe8cd5f",
        "load_empty": "88f22fb0cf359078bc7311021aefcfba8ef529661fc3d25b0f909d95af66d7f7",
        "load_reports": "ee01b675844d631c42f36f992843307941cdf340d1cc19bc536e96ee06a12c34",
        "load_help": "3fbe0b3a6c0027bec7a396f9a003733d2ae18edbf024b851d231185dbff621f6",
        "map_drawing": "28432545eb8e4bedebec62c6e90c532392e280296beaf647776d8c5637255611",
        "polygon_scan": "3918fe5228a20e17db54894e1b5c9a93f4ed6c22303e06719bb4eac13026a481",
        "polygon_scan_bad": "27d1d04aa85986062e3adc9a28e582196a435d0aaafe33232c5ae1a2c137535a",
        "mission_drawing": "7cf73d8cd92f057314e42f0aa949df4a33c457efd28791c26b477feb3c77e6e5",
        "rhm_inventory": "9ae072edd850cf1e067e8fac950cf6bb33b4db6c435c9775fd7352e778253ea0",
        "rhm_probe": "4f4fd5c838882ff18b01f27c77487b2ef37a7400479b22935891fd0c0c598ef9",
        "rhm_full": "85519694f1880aea22a02d4049335cbe908cfe68f2e7ecd010230d44d85fcdd9",
        "profiles": "7ca904b5abfb1a04280e86cfe2459d3893947ac495e0aaa5e2052d459312935e",
        "profiles--scot": "3b0e3cd2846e8293e639676e9b2e5b3ce06887bfbf0d6a303ba56e109c75e25b",
        "profiles--cast": "a448dab10235b3b3a17af0a0104cd8e4a3c2b9d0f4b6e769ae962417dee72845",
        "rhm_records": "a1d35c3ea9935ef798ad16ec65046f578c4ce04c4bee5b27cfee038a19247e42",
        "scb_probe": "0fee333a37901016c86473db063e45d2f12e2d13d96b3855dce846642b0bd047",
        "scb_opstats": "b73b0868ec68c6a7574ce6e734eddda3513c10ad4e3597f29904fc421957599b",
        "natives": "031f4f5ea318a2efa683965087709d4eabf21a9e145b741a39e24b929d4fab9e",
        "natives_filter": "45dac2b07f131fd5593bc6ec578cecde0057d4c93219938190b6d9515c72772d",
        "ops": "48687b3588fe81f1af38d415553c58dde2ca15e18901e94d6c90b53c361e23f8",
        "imm": "42388179cfb1f176b38818be0e0a34524699e1bbd6a89239ab994f158c37c663",
        "handlers": "d44ded234f8ac05644cc2dcd64bb48b4b7ef4164c7368e6ba83f2c7190dbc18c",
        "find": "1bec141725f4afe844986501244aa9de12cb8038c09fb2486fbb3ef0bbe76731",
        "messages": "a69ea08032a5983d881a07aa1b92695b45e1f84f85ffd7706ced4d524f4b43e6",
        "params": "c086a2613e454344cb6754c8956b4cc3c773574130d5db7b0754f601fee3880d",
        "cmd_pseudo": "4d3bc386fcd2a348f6e59f9de31a3ffc6e420545ed1be990d0b5f09474bc78c5",
        "sprite_decode": "c52e327938a816a8e42f74225014322e26cea827510f330b5c918cbe934c50ab",
        "stat_layers": "69ffdd1e2043b1213e77dfb0179f74477dc4d891bd1c52c02f393865d9862f1e",
        "stat_layers_quiet": "774b3ae6f9c74a2c9ec32784888b8d4f81c33b70da50d3627866f4bb354b6a8e",
        "xref": "a8c627e4d964d944b03668f01495965873febb7afc3ef4386ea0a2251619f140",
    }

    def snapshot(self, name, action, events=None):
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            try:
                result = action()
                error = None
            except Exception as exc:  # noqa: BLE001 -- characterize the tool's exception
                result = None
                error = (type(exc).__name__, str(exc))
        record = stable((output.getvalue(), result, error, events() if events else None))
        digest = hashlib.sha256(json.dumps(record, ensure_ascii=True).encode()).hexdigest()
        with self.subTest(snapshot=name):
            self.assertEqual(self.SNAPSHOTS.get(name), digest, f"{name}: {digest}\n{record}")

    def fake_files(self, module, classes=None, data=None):
        stack = contextlib.ExitStack()
        self.addCleanup(stack.close)
        stack.enter_context(patch("builtins.open", mock_open(read_data=b"synthetic")))
        if classes is not None:
            stack.enter_context(patch.object(module, "parse", return_value=(1.5, classes, 9)))
        if data is not None:
            stack.enter_context(patch.object(module, "parse_file", return_value=data))
        return stack

    def test_flow_and_pseudo(self):
        mod = tool("scb_semantics")
        cls = script_class()
        events = []
        flow = mod.Flow(cls, 0, cls["nquads"])
        self.snapshot("flow", lambda: flow.walk(
            lambda *a: events.append(("native", a)), lambda *a: events.append(("use", a))),
            lambda: (events, flow.writer))
        self.snapshot("pseudo", lambda: mod.pseudo(cls, 0, cls["nquads"], {19: "Initialize"}))
        self.snapshot("flow_without_callbacks", lambda: mod.Flow(cls, 0, cls["nquads"]).walk())
        bad = {**cls, "quads": quad(0x13), "nquads": 2}
        self.snapshot("flow_truncated", lambda: mod.Flow(bad, 0, 2).walk())

    def test_semantics_reports(self):
        mod = tool("scb_semantics")
        self.fake_files(mod, [script_class(), script_class("Element")])
        actions = {
            "natives": lambda: mod.cmd_natives(["sample.scb"]),
            "natives_filter": lambda: mod.cmd_natives(["sample.scb"], {3}),
            "ops": lambda: mod.cmd_ops(["sample.scb"]),
            "imm": lambda: mod.cmd_imm(["sample.scb"]),
            "handlers": lambda: mod.cmd_handlers(["sample.scb"]),
            "find": lambda: mod.cmd_find(["sample.scb"], [0x29, 0xFE], 1),
            "messages": lambda: mod.cmd_messages(["sample.scb"]),
            "params": lambda: mod.cmd_params(["sample.scb"] * 3),
            "cmd_pseudo": lambda: mod.cmd_pseudo("sample.scb", "Element", "Initialize"),
        }
        for name, action in actions.items():
            with self.subTest(name=name):
                self.snapshot(name, action)

    def test_load_natives_and_reports(self):
        mod = tool("scb_load_natives")
        cls = script_class()
        other = script_class("Element")
        other["funcs"][0] = ("PostInitialize", *other["funcs"][0][1:])
        self.snapshot("load_natives", lambda: mod.load_natives([cls, other],
                      ("Initialize", "PostInitialize", "ProcessMessage", "missing")))
        self.snapshot("load_empty", lambda: mod.load_natives([], ()))
        stack = self.fake_files(mod, [cls, other])
        stack.enter_context(patch.object(mod, "files_of", return_value=["sample.scb"]))
        options = ["--missions", "--ids", "--all-unknown", "--context", "--known", "3", "--limit", "1", "--id", "43", "99"]
        with patch.object(sys, "argv", ["tool", "fake", *options]):
            self.snapshot("load_reports", mod.main)
        with patch.object(sys, "argv", ["tool"]):
            self.snapshot("load_help", mod.main)

    def test_script_cli_statistics(self):
        for name in ("scb_probe", "scb_opstats"):
            with self.subTest(name=name):
                mod = tool(name)
                with contextlib.ExitStack() as stack:
                    stack.enter_context(patch("builtins.open", mock_open(read_data=b"synthetic")))
                    stack.enter_context(patch.object(mod, "parse", return_value=(1.5, [script_class()], 8)))
                    stack.enter_context(patch.object(mod.os.path, "isdir", return_value=True))
                    stack.enter_context(patch.object(mod.glob, "glob", return_value=["b.scb", "a.scb"]))
                    stack.enter_context(patch.object(sys, "argv", ["tool", "fake", "--dump", "StartUp"]))
                    self.snapshot(name, mod.main)

    def test_rhm_records(self):
        mod = tool("scb_elements")
        text = (
            "BORG v1: 2\n  item first_80000000\n  unnamed\nSKRO: 1\n  scroll second_80000001\n"
            "GULP: 1 points, 2 script polygons\n  3 pts area_80000002\n  4 pts other_80000003\n"
            'RAIL: 1\n  rail 2: "point__1___80000004" "point__2___80000005"\n'
            "ZORG: 1\n  entry mark_80000006\nCAVE: 2\n"
            "HIRN: 1 waypoints, 2 bushes, 3 beam-me points\n  ignored_80000007\n"
            "POUF: 4\nTING: 5\nMEOW v3: 1\n  thing final_80000008")
        with patch.object(mod.subprocess, "run", return_value=Mock(stdout=text)) as run:
            self.snapshot("rhm_records", lambda: mod.rhm_records("tool", "fake.rhm"), lambda: run.call_args_list)

    def test_elements_main(self):
        mod = tool("scb_elements")
        stack = self.fake_files(mod, [script_class(), script_class("unit"), script_class("unknown")])
        stack.enter_context(patch.object(mod.os, "listdir", return_value=["b.txt", "unit.scb", "skip.scb"]))
        stack.enter_context(patch.object(mod.os.path, "exists", return_value=True))
        stack.enter_context(patch.object(mod, "rhm_records", return_value=({"unit": ("BORG", 2)}, {"BORG": 3})))
        with patch.object(sys, "argv", ["tool", "levels", "exe", "unit"]):
            self.snapshot("elements_main", mod.main)

    def test_xref_main(self):
        mod = tool("scb_xref")
        stack = self.fake_files(mod, [script_class()])
        stack.enter_context(patch.object(mod, "RED_CODE", {"fake": "ZZ", "skip": "YY"}))
        stack.enter_context(patch.object(mod.os.path, "exists", side_effect=lambda p: "skip" not in p))
        stack.enter_context(patch.object(mod, "rhm_counts", return_value={"actors": 2}))
        stack.enter_context(patch.object(mod, "red_text_count", return_value=(3, 1, 2)))
        with patch.object(sys, "argv", ["tool", "levels", "texts", "exe", "3", "43"]):
            self.snapshot("xref", mod.main)

    def test_cpf_reports(self):
        rec = {"sprite": "synthetic", "sequence": "seq", "label": "label", "voice": "v",
               "pre": "00", "post": "01"}
        data = {"pc": [rec], "sd": [rec] * 6, "cv": [rec],
                "level": [{"code": "ZZ", "map": "map", "mission": "mission", "title": "title", "music": 1, "mid": "00"}],
                "table_a": [{"head": "00" * 28, "records": ["00" * 32]}], "table_b": ["00" * 81]}
        for name in ("cpf_probe", "cpf_stats"):
            mod = tool(name)
            with self.subTest(name=name), contextlib.ExitStack() as stack:
                stack.enter_context(patch("builtins.open", mock_open(read_data=b"synthetic")))
                stack.enter_context(patch.object(mod, "parse", return_value=data))
                if name == "cpf_stats":
                    stack.enter_context(patch.object(mod, "FAMILIES", ["family"]))
                    rows = [(dict.fromkeys(mod.SD_PRE, i), dict.fromkeys(mod.SD_POST, 1), (1.0,), (2.0,)) for i in range(6)]
                    stack.enter_context(patch.object(mod, "sd_fields", side_effect=rows))
                    stack.enter_context(patch.object(mod, "pc_fields", return_value=((1, 2), (3, 4))))
                self.snapshot(name, lambda mod=mod: mod.main(["sample.cpf", "--hex", "--tiers", "--tables", "--json", "out.json"]))
        mod = tool("cpf_probe")
        with patch("builtins.open", mock_open(read_data=b"synthetic")), patch.object(mod, "parse", return_value=data), patch.object(mod.os.path, "exists", return_value=False):
            self.snapshot("cpf_missing_rhs", lambda: mod.main(["sample.cpf", "--rhs", "fake"]))

    def test_mission_reports(self):
        for name in ("rhm_full", "rhm_profiles"):
            mod = tool(name)
            with self.subTest(name=name), contextlib.ExitStack() as stack:
                stack.enter_context(patch("builtins.open", mock_open(read_data=b"synthetic")))
                stack.enter_context(patch.object(mod, "parse_file", side_effect=[ValueError("bad container"), mission()]
                                              if name == "rhm_full" else None, return_value=mission()))
                stack.enter_context(patch.object(mod.os.path, "isdir", return_value=True))
                stack.enter_context(patch.object(mod.glob, "glob", return_value=["b.rhm", "a.rhm"]))
                if name == "rhm_full":
                    stack.enter_context(patch.object(sys, "argv", ["tool", "fake", "--stats", "--json", "out.json"]))
                    self.snapshot(name, mod.main)
                else:
                    cpf = {key: [{"sprite": "s", "label": "l", "voice": "v"}] for key in ("sd", "pc", "cv")}
                    stack.enter_context(patch.object(mod, "parse_cpf", return_value=cpf))
                    for option in ([], ["--scot"], ["--cast", "a", "b"]):
                        self.snapshot("profiles" + (option[0] if option else ""), lambda mod=mod, option=option: mod.main(["fake", "cpf", *option]))

    def test_mission_group_reports(self):
        group = struct.pack("<4sIIH", b"BORG", 6, 1, 0)
        body = b"\x01\x00" + group
        for name in ("rhm_inventory", "rhm_probe"):
            mod = tool(name)
            with self.subTest(name=name), contextlib.ExitStack() as stack:
                stack.enter_context(patch("builtins.open", mock_open(read_data=b"synthetic")))
                stack.enter_context(patch.object(mod, "children", return_value=(None, None, [("BOYZ", 1, 0, body), ("NONE", 2, 0, b"x")])))
                stack.enter_context(patch.object(mod.glob, "glob", return_value=["b.rhm", "a.rhm"]))
                stack.enter_context(patch.object(mod.os.path, "isdir", return_value=True))
                if name == "rhm_probe":
                    stack.enter_context(patch.object(mod, "parse_group", return_value=([{"x": 1, "flag": 2}], "left bytes")))
                stack.enter_context(patch.object(sys, "argv", ["tool", "fake", "--dump", "BORG"]))
                self.snapshot(name, mod.main)

    def test_chunk_reports(self):
        mod = tool("rhm_chunks_probe")
        tags = ("RAIL", "GULP", "SKRO", "ZORG", "CAVE", "HIRN", "TING", "FOOT")
        with contextlib.ExitStack() as stack:
            stack.enter_context(patch("builtins.open", mock_open(read_data=b"synthetic")))
            stack.enter_context(patch.object(mod.glob, "glob", return_value=["sample.rhm"]))
            stack.enter_context(patch.object(mod, "children", return_value=(None, None, [(tag, 1, 0, b"\x00\x00xyz") for tag in tags])))
            stack.enter_context(patch.object(mod, "parse_rail", return_value=[[(1, 2, b"abcde", b"x" * 22), (3, 4, b"abcde", b"")]]))
            stack.enter_context(patch.object(mod, "parse_gulp", return_value=([(1, 2)], [(1, [1, 2], 3, "name")])))
            stack.enter_context(patch.object(mod, "parse_skro", side_effect=ValueError("bad skro")))
            stack.enter_context(patch.object(mod, "parse_zorg", return_value=[]))
            stack.enter_context(patch.object(mod, "parse_hirn", return_value={"BUSH": (1, b"\x02\x00x"), "NLIP": (2, b"x")}))
            stack.enter_context(patch.object(sys, "argv", ["tool", "fake", *tags]))
            self.snapshot("chunks", mod.main)

    def test_mission_drawing(self):
        mod = tool("rhm_overlay")
        draw = Mock()
        image = Mock()
        with patch("builtins.open", mock_open()), patch.object(mod.json, "load", return_value=mission()), patch.object(mod.Image, "open", return_value=Mock(convert=Mock(return_value=image))), patch.object(mod.ImageDraw, "Draw", return_value=draw), patch.object(sys, "argv", ["tool", "mission.json", "map.png", "out.png"]):
            self.snapshot("mission_drawing", mod.main, lambda: (draw.mock_calls, image.mock_calls))

    def test_map_drawing_and_scan(self):
        mod = tool("overlay")
        draw = Mock()
        image = Mock(width=20, height=30, size=(20, 30))
        image.crop.return_value = image
        poly = [(1, 1), (4, 1), (1, 4)]
        with contextlib.ExitStack() as stack:
            stack.enter_context(patch.object(mod, "load_chunks", return_value={tag: (1, b"") for tag in ("STAT", "WOAW", "007 ", "PPPP")}))
            stack.enter_context(patch.object(mod, "load_map", return_value=Mock(convert=Mock(return_value=image))))
            stack.enter_context(patch.object(mod, "map_path", return_value="map.png"))
            stack.enter_context(patch.object(mod.ImageDraw, "Draw", return_value=draw))
            stack.enter_context(patch.object(tool("probe_stat3"), "parse_stat", return_value=([[(0, 2, 1, poly)], [(0, 0, 0, [])]], 0)))
            stack.enter_context(patch.object(mod, "woaw_areas", return_value=[([(x, y, 0, 0) for x, y in poly], (), (), (), 0, 0)]))
            stack.enter_context(patch.object(mod, "bonds", return_value=[(1, 2, 3, 4, 5, 6, 7)]))
            stack.enter_context(patch.object(mod, "pppp", return_value=([(1, poly, b"")], 0)))
            stack.enter_context(patch.object(sys, "argv", ["tool", "map", "out.png", "stat,woaw,bond,pppp", "0", "0", "10", "10"]))
            self.snapshot("map_drawing", mod._main, lambda: (draw.mock_calls, image.mock_calls))
        polygon = struct.pack("<H6H", 3, 1, 1, 4, 1, 1, 4)
        self.snapshot("polygon_scan", lambda: mod.stat_polys_scan(b"\xff\xff" + polygon * 3, 20, 30))
        self.snapshot("polygon_scan_bad", lambda: mod.stat_polys_scan(b"\xff" * 25, 1, 1))

    def test_stat_layers(self):
        mod = tool("probe_stat_layers")
        poly = struct.pack("<BH6HB", 1, 3, 1, 1, 4, 1, 1, 4, 2)
        body = b"\x03" + poly + struct.pack("<HIHHHH", 0, 0, 0, 2, 0, 7) + b"bad!"
        with patch.object(mod, "load_chunks", return_value={"STAT": (1, body)}), patch.object(mod, "map_size", return_value=(20, 30)), patch.object(mod, "decoded_prefix", return_value=((2,), [(0, 0)], [], 0)):
            self.snapshot("stat_layers", lambda: mod.sequential("map", (2, 2)))
            self.snapshot("stat_layers_quiet", lambda: mod.sequential("map", verbose=False))

    def test_animation_modes(self):
        mod = tool("anim_sheet")
        frame = {"frame": 1, "anchor": (150, 150), "duration": 0x10002}
        anims = [{"frames": [frame] * (1 + i % 2), "u04": 1, "u08": 2, "u0c": 3} for i in range(32)]
        for args in ([], ["--strip", "0", "1"], ["--blocks", "0:2", "--dir", "1"]):
            with self.subTest(args=args), contextlib.ExitStack() as stack:
                stack.enter_context(patch.object(mod, "parse_rhs", return_value=[{"anims": anims, "name": "synthetic"}]))
                stack.enter_context(patch.object(mod, "Bank", return_value="bank"))
                stack.enter_context(patch.object(mod, "load_pages", return_value="pages"))
                render = stack.enter_context(patch.object(mod, "render_cells", return_value=np.zeros((2, 3, 4), dtype=np.uint8)))
                write = stack.enter_context(patch.object(mod, "write_png"))
                stack.enter_context(patch.object(mod.os, "makedirs"))
                stack.enter_context(patch.object(sys, "argv", ["tool", "sample.rhs", "out", *args]))
                self.snapshot("anim" + (args[0] if args else ""), mod.main, lambda render=render, write=write: (render.call_args_list, write.call_args_list))

    def test_sprite_decode(self):
        bank = Mock(width=np.array([], dtype=int), height=np.array([], dtype=int))
        bank.frames_of_page.return_value = np.array([], dtype=int)
        with patch.object(tool("spritebank"), "Bank", return_value=bank), contextlib.redirect_stdout(io.StringIO()):
            mod = tool("sprite_stats6")
        cases = [([], 2, 1, False), ([1], 2, 1, False), ([0, 0xFFFF], 5, 1, False),
                 ([0, 2, 7, 8], 2, 1, False), ([0, 1, 7, 8], 2, 1, True),
                 ([1, 3, 7, 8, 9], 2, 1, False), ([0, 0], 1, 1, False), ([1, 0], 1, 1, False)]
        self.snapshot("sprite_decode", lambda: [mod.decode(*case) for case in cases])


class OriginalTests(unittest.TestCase):
    SNAPSHOTS = ProbeTests.SNAPSHOTS
    snapshot = ProbeTests.snapshot

    @classmethod
    def setUpClass(cls):
        import ctypes
        sys.path.insert(0, str(PROBES.parent / "original"))
        modules = {name: Mock() for name in ("pyautogui", "win32con", "win32gui", "win32process")}
        with patch.dict(sys.modules, modules), patch.object(ctypes, "windll", Mock(), create=True), patch.object(sys, "stdout", Mock()):
            cls.rhcap = importlib.import_module("rhcap")
            cls.oracle = importlib.import_module("oracle_input")
            cls.frames = importlib.import_module("frame_rec")

    def test_frame_commands(self):
        mod = self.frames
        commands = [[], ["rec", "out", "2"], ["rec", "out", "2", "10", "0", "1", "2", "3"],
                    ["save", "in", "0", "out"], ["blobs", "in"],
                    ["blobs", "in", "0", "1", "2", "3", "12", "4"],
                    ["track", "in", "0", "1", "2", "3"], ["changes", "in", "0", "1", "2", "3", "5"],
                    ["unknown", "in"], ["track", "in", "bad"]]
        for i, args in enumerate(commands):
            with self.subTest(args=args), contextlib.ExitStack() as stack:
                events = Mock()
                for name, result in {"record": (np.array([0.0, 0.5, 1.0]), np.zeros((3, 2, 2))),
                                     "load": (np.array([0.0]), np.zeros((1, 2, 2))),
                                     "blobs": [(0.0, []), (1.0, [(1, 2, 3, 4, 2.0, 4, 8)])],
                                     "track": [(0.0, None), (1.0, 2.0, 4, 1, 2, 3, 4, 8)],
                                     "fit_speed": None, "changes": [(0.0, -1), (1.0, 3)]}.items():
                    mocked = stack.enter_context(patch.object(mod, name, return_value=result))
                    events.attach_mock(mocked, name)
                from PIL import Image
                image = Mock()
                mocked = stack.enter_context(patch.object(Image, "fromarray", return_value=image))
                events.attach_mock(mocked, "image")
                self.snapshot(f"frames{i}", lambda args=args: mod.main(args), lambda events=events, image=image: (events.mock_calls, image.mock_calls))

    def test_oracle_commands(self):
        mod = self.oracle
        commands = [[], ["probe"], ["move", "1", "2"], ["click", "1", "2"],
                    ["click", "1", "2", "right", "3"], ["mmove", "1", "2"],
                    ["mclick", "1", "2", "right", "2"], ["rec", "out", "1", "2"], ["unknown"]]
        for i, args in enumerate(commands):
            with self.subTest(args=args), contextlib.ExitStack() as stack:
                events = Mock()
                for name, result in {"clamp_origin": None, "phys": (1, 2), "gmove": (2, 3), "gclick": None,
                                     "mmove": ((1, 2), 0.9, 0.6), "mclick": ((1, 2), 0.9, 0.6),
                                     "record": [(0.5, "frame.png")]}.items():
                    events.attach_mock(stack.enter_context(patch.object(mod, name, return_value=result)), name)
                for name, result in {"find_window": (5, "window"), "focus": True, "raw_rel": None}.items():
                    events.attach_mock(stack.enter_context(patch.object(mod.rhcap, name, return_value=result)), name)
                events.attach_mock(stack.enter_context(patch.object(mod.time, "sleep")), "sleep")
                self.snapshot(f"oracle{i}", lambda args=args: mod.main(args), lambda events=events: events.mock_calls)

    def test_closed_loop_cursor(self):
        mod = self.oracle
        events = Mock()
        image = Mock()
        readings = [((0, 0), 0.1), ((10, 10), 0.9), ((30, 30), 0.8), ((40, 50), 0.99)]
        with contextlib.ExitStack() as stack:
            stack.enter_context(patch.object(mod.rhcap, "CAPDIR", "captures"))
            for target, name, result in [(mod.rhcap, "find_window", (5, "w")), (mod.rhcap, "grab", image),
                                          (mod.rhcap, "raw_rel", None), (mod.time, "sleep", None), (mod.time, "time", 1.0)]:
                events.attach_mock(stack.enter_context(patch.object(target, name, return_value=result)), name)
            stack.enter_context(patch.object(mod, "find_cursor", side_effect=readings))
            self.snapshot("mmove", lambda: mod.mmove(40, 50, verbose=True), lambda: (events.mock_calls, image.mock_calls))
            self.snapshot("mmove_zero", lambda: mod.mmove(1, 2, h=5, iters=0))

    def test_capture_commands(self):
        mod = self.rhcap
        commands = [[], ["seq", "; move 1 2; sleep 0.5; key escape;"], ["launch", "fake"], ["kill"],
                    ["win"], ["shot", "name"], ["boot", "out", "1", "0.2"], ["move", "1", "2"],
                    ["rclick", "1", "2"], ["click", "1", "2", "right", "3"], ["focus"],
                    ["drag", "1", "2", "3", "4", "right"], ["key", "escape", "0.1"],
                    ["key", "unknown", "0.2"], ["key", "unknown"], ["type", "abc"],
                    ["hover", "1", "2", "name"], ["pix", "1", "2"], ["unknown"], ["move", "bad", "2"]]
        for i, args in enumerate(commands):
            with self.subTest(args=args), contextlib.ExitStack() as stack:
                events = Mock()
                image = Mock(size=(20, 30))
                image.tobytes.return_value = b"synthetic"
                image.getpixel.return_value = (1, 2, 3)
                for name, result in {"need_window": (5, "window"), "find_window": (5, "window"),
                                     "client_rect": (1, 2, 20, 30), "shot": "out.png", "grab": image,
                                     "focus": True, "rmove": None, "rclick": None, "skey": None}.items():
                    events.attach_mock(stack.enter_context(patch.object(mod, name, return_value=result)), name)
                gui = Mock()
                stack.enter_context(patch.object(mod, "pyautogui", gui))
                stack.enter_context(patch.object(mod, "win32process", Mock(GetWindowThreadProcessId=Mock(return_value=(1, 42)))))
                stack.enter_context(patch.object(mod, "win32gui", Mock(GetForegroundWindow=Mock(return_value=5))))
                stack.enter_context(patch.object(mod.subprocess, "Popen", return_value=Mock(pid=42)))
                events.attach_mock(stack.enter_context(patch.object(mod.subprocess, "call", return_value=0)), "process")
                stack.enter_context(patch.object(mod.subprocess, "DETACHED_PROCESS", 8, create=True))
                stack.enter_context(patch.object(mod.subprocess, "CREATE_NEW_PROCESS_GROUP", 512, create=True))
                stack.enter_context(patch.object(mod.os, "makedirs"))
                stack.enter_context(patch.object(mod, "CAPDIR", "captures"))
                events.attach_mock(stack.enter_context(patch.object(mod.time, "sleep")), "sleep")
                stack.enter_context(patch.object(mod.time, "time", side_effect=[0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 1.1]))
                self.snapshot(f"capture{i}", lambda args=args: mod.main(args), lambda events=events, gui=gui, image=image: (events.mock_calls, gui.mock_calls, image.mock_calls))


if __name__ == "__main__":
    unittest.main()
