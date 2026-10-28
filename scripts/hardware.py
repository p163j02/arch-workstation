#!/usr/bin/env python3
"""Package names for the detected CPU and display adapters."""
from pathlib import Path


def packages(cpuinfo=Path('/proc/cpuinfo'), pci=Path('/sys/bus/pci/devices')):
    result = set()
    cpu = cpuinfo.read_text()
    if 'AuthenticAMD' in cpu:
        result.add('amd-ucode')
    elif 'GenuineIntel' in cpu:
        result.add('intel-ucode')
    for device in pci.iterdir():
        if not (device / 'class').read_text().strip().startswith('0x03'):
            continue
        vendor = (device / 'vendor').read_text().strip()
        if vendor == '0x1002':
            result.update(['mesa', 'lib32-mesa', 'vulkan-radeon', 'lib32-vulkan-radeon'])
        elif vendor == '0x8086':
            result.update(['mesa', 'lib32-mesa', 'vulkan-intel', 'lib32-vulkan-intel'])
        elif vendor == '0x10de':
            result.update(['nvidia-open-dkms', 'nvidia-utils', 'lib32-nvidia-utils', 'nvidia-settings'])
    return sorted(result)


if __name__ == '__main__':
    print('\n'.join(packages()))
