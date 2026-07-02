#!/bin/bash
# Run on the target server to roll the staged jar and restart via systemd.
set -e
systemctl restart nubase
systemctl --no-pager status nubase | head -n 5 || true
