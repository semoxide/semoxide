"""semoxide process plugin in plain Python: stdlib only, no SDK.

Implements analyzeCommits + generateNotes; declares no publish.
"""

import json
import sys

# Windows: use the binary streams to avoid cp1252 and "\r\n" translation.
stdin = sys.stdin.buffer
stdout = sys.stdout.buffer


def send(msg):
    stdout.write(json.dumps(msg).encode("utf-8") + b"\n")
    stdout.flush()


def log(level, message):
    send({"jsonrpc": "2.0", "method": "log", "params": {"level": level, "message": message}})


def analyze(ctx):
    rank = {None: 0, "patch": 1, "minor": 2, "major": 3}
    best = None
    for c in ctx["commits"]:
        msg = c["message"]
        head = msg.splitlines()[0] if msg else ""
        t = None
        if "BREAKING CHANGE:" in msg or head.split(":")[0].endswith("!"):
            t = "major"
        elif head.startswith("feat"):
            t = "minor"
        elif head.startswith(("fix", "perf")):
            t = "patch"
        if rank[t] > rank[best]:
            best = t
    log("info", f"python analyzed {len(ctx['commits'])} commits -> {best}")
    return best


HANDLERS = {
    "analyzeCommits": analyze,
    "generateNotes": lambda ctx: "### From Python\n- prior notes: %d chars" % len(ctx["nextRelease"]["notes"]),
    "bench": lambda ctx: None,
}

for raw in stdin:
    msg = json.loads(raw)
    method = msg.get("method")
    if "id" not in msg:
        if method == "shutdown":
            break
        continue
    if method == "initialize":
        if msg["params"]["protocol"] != 1:
            send({"jsonrpc": "2.0", "id": msg["id"], "error": {"code": -32001, "message": "unsupported protocol"}})
            continue
        result = {"protocol": 1, "name": "python-demo", "steps": ["analyzeCommits", "generateNotes"]}
    elif method in HANDLERS:
        result = HANDLERS[method](msg["params"].get("context"))
    else:
        send({"jsonrpc": "2.0", "id": msg["id"], "error": {"code": -32601, "message": f"method not found: {method}"}})
        continue
    send({"jsonrpc": "2.0", "id": msg["id"], "result": result})
