#!/usr/bin/env python3
import json
import sys


def main() -> int:
    raw = sys.stdin.read().strip()
    if not raw:
        print(json.dumps({"success": True, "payload": {"summary": ""}, "metrics": [], "artifacts": []}))
        return 0

    request = json.loads(raw)
    payload = request.get("input", {})
    text = payload.get("source_text", "")
    summary = text[:120].strip()
    print(json.dumps({
        "success": True,
        "payload": {"summary": summary},
        "metrics": [{"name": "chars_in", "value": float(len(text))}],
        "artifacts": []
    }))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
