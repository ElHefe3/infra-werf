#!/bin/sh
set -eu
umask 000
: > /tmp/atlas-control-count
chmod 666 /tmp/atlas-control-count
ssh-keygen -q -t ed25519 -N '' -f /etc/ssh/ssh_host_ed25519_key
exec /usr/sbin/sshd -D -e -f /etc/atlas-sshd-config
