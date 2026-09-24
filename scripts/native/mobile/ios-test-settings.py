"""Prepare only probe endpoints, never profiles or subscriptions, in the owned simulator."""
import json
import pathlib
import plistlib
import sqlite3
import sys

mode, source, value = sys.argv[1:4]
if mode == "xctestrun":
    path = pathlib.Path(source)
    with path.open("rb") as handle:
        data = plistlib.load(handle)
    env = json.loads(value)
    def visit(node):
        if isinstance(node, dict):
            if "TestBundlePath" in node:
                node.setdefault("EnvironmentVariables", {}).update(env)
            for child in list(node.values()):
                visit(child)
        elif isinstance(node, list):
            for child in node:
                visit(child)
    visit(data)
    with path.open("wb") as handle:
        plistlib.dump(data, handle)
elif mode == "database":
    paths = list(pathlib.Path(source).rglob("voyavpn.sqlite"))
    if len(paths) != 1:
        raise RuntimeError(f"Expected one initialized database, found {paths}")
    with sqlite3.connect(paths[0]) as connection:
        settings = json.loads(connection.execute("SELECT payload FROM app_settings WHERE id=1").fetchone()[0])
        settings["speedTest"].update(timeoutSeconds=3, latencyUrl=value + "/ping", ipLookupUrl=value + "/ip")
        connection.execute("UPDATE app_settings SET payload=? WHERE id=1", (json.dumps(settings),))
else:
    raise RuntimeError("Unknown mode")
