#!/bin/sh
# Lummo was called Lumio before 0.1.15. This keeps the old command working,
# including for Lumio itself: after installing an update it restarts through
# /usr/bin/lumio.
exec /usr/bin/lummo "$@"
