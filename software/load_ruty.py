# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
# Load the RUTY image on the AUP-ZU3 (PYNQ).  sudo -E env PATH=$PATH python3 load_ruty.py [ruty.bit]
# This replaces whatever is in the FPGA fabric until the next overlay load or reboot. Linux keeps running.
import sys
from pynq import Overlay
bit = sys.argv[1] if len(sys.argv) > 1 else "ruty.bit"
ol = Overlay(bit)           # needs ruty.hwh next to the .bit; also sets the fabric clock the image was built for
print("loaded", bit)
print("address map:", {k: hex(v["phys_addr"]) for k, v in ol.ip_dict.items()})
from pynq import Clocks
print("fabric clock 0: %.1f MHz" % Clocks.fclk0_mhz)
