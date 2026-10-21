#!/bin/sh
# Remove the polkit rule tunelito-priv wrote after the first approved setup.
# The tunelito account and the sshd drop-in are left for the administrator.
set -e
rm -f /etc/polkit-1/rules.d/49-tunelito.rules
