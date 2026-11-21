#!/usr/bin/env python3
"""Capture deterministic Linux route, link, rule and resolver state."""
import hashlib
import subprocess
from pathlib import Path


def network_state():
    result = {}
    for name, args in {'routes': ['route'], 'links': ['link'], 'rules': ['rule']}.items():
        result[name] = subprocess.check_output(['ip', '-j', *args], text=True)
    result['resolv_conf_sha256'] = hashlib.sha256(Path('/etc/resolv.conf').read_bytes()).hexdigest()
    return result
