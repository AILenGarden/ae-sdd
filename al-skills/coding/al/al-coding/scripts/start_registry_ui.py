#!/usr/bin/env python3
"""Launch the ALCoding registry UI and open it in the default browser."""

from __future__ import annotations

import argparse
import threading
import time
import webbrowser
from pathlib import Path

from registry_ui import RegistryServer, serve_server


def main() -> int:
    parser = argparse.ArgumentParser(description="Launch the ALCoding registry UI")
    parser.add_argument("--root", default=".", help="ALCoding root")
    parser.add_argument("--host", default="127.0.0.1", help="bind host")
    parser.add_argument("--port", type=int, default=8765, help="bind port (0 chooses a free port)")
    parser.add_argument("--no-browser", action="store_true", help="do not open a browser")
    args = parser.parse_args()
    server = RegistryServer((args.host, args.port), Path(args.root).resolve())
    if not args.no_browser:
        url = f"http://{server.server_address[0]}:{server.server_port}/"
        threading.Thread(target=lambda: (time.sleep(0.5), webbrowser.open(url)), daemon=True).start()
    serve_server(server)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
