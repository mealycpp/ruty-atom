# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (c) 2026 Mohamed El-Hadedy
"""ruty_se2.py : gem5 SE-mode model of the AUP-ZU3 application processor, with the knobs needed to CALIBRATE it against the real Cortex-A53.
    build/ARM/gem5.opt -d <outdir> ruty_se2.py --cmd <binary> --options "<args>" [--cpu minor|hpi|o3] [--clock 1.2GHz]
        [--l1pf N] [--l2pf N] [--l1lat N] [--l2lat N] [--mem DDR4_2400_8x8|DDR4_2400_16x4|simple] [--memlat 30ns]
The stock model (no prefetcher, 12-cycle L2) copies memory about 4 times slower than the board; the options below close that gap."""
import argparse, m5, sys, os
from m5.objects import *
sys.path.insert(0, os.path.join(os.getcwd(), 'configs'))
ap = argparse.ArgumentParser()
ap.add_argument("--cmd", required=True); ap.add_argument("--options", default="")
ap.add_argument("--clock", default="1.2GHz"); ap.add_argument("--cpu", default="minor", choices=["minor", "hpi", "o3", "timing"])
ap.add_argument("--l1pf", type=int, default=0); ap.add_argument("--l2pf", type=int, default=0)
ap.add_argument("--l1lat", type=int, default=2); ap.add_argument("--l2lat", type=int, default=12)
ap.add_argument("--mem", default="DDR4_2400_8x8"); ap.add_argument("--memlat", default="30ns")
ap.add_argument("--wb", type=int, default=8)
args = ap.parse_args()
if args.cpu == "hpi":
    from common.cores.arm import HPI
    CPU = HPI.HPI
else:
    CPU = {"minor": ArmMinorCPU, "o3": ArmO3CPU, "timing": ArmTimingSimpleCPU}[args.cpu]
class L1(Cache):
    assoc = 4; tag_latency = args.l1lat; data_latency = args.l1lat; response_latency = 1; mshrs = 8; tgts_per_mshr = 8; write_buffers = args.wb
class L2(Cache):
    size = "1MB"; assoc = 16; tag_latency = args.l2lat; data_latency = args.l2lat; response_latency = 2; mshrs = 16; tgts_per_mshr = 8; write_buffers = 16
system = System()
system.clk_domain = SrcClockDomain(clock=args.clock, voltage_domain=VoltageDomain())
system.mem_mode = "timing"; system.mem_ranges = [AddrRange("512MB")]
system.cpu = CPU()
system.cpu.icache = L1(size="32kB", assoc=2); system.cpu.dcache = L1(size="32kB")
if args.l1pf: system.cpu.dcache.prefetcher = StridePrefetcher(degree=args.l1pf)
system.cpu.icache.cpu_side = system.cpu.icache_port; system.cpu.dcache.cpu_side = system.cpu.dcache_port
system.l2bus = L2XBar()
system.cpu.icache.mem_side = system.l2bus.cpu_side_ports; system.cpu.dcache.mem_side = system.l2bus.cpu_side_ports
system.l2 = L2()
if args.l2pf: system.l2.prefetcher = StridePrefetcher(degree=args.l2pf)
system.l2.cpu_side = system.l2bus.mem_side_ports
system.membus = SystemXBar(); system.l2.mem_side = system.membus.cpu_side_ports
system.cpu.createInterruptController(); system.system_port = system.membus.cpu_side_ports
if args.mem == "simple":
    system.mem_ctrl = SimpleMemory(range=system.mem_ranges[0], latency=args.memlat, bandwidth="12GiB/s"); system.mem_ctrl.port = system.membus.mem_side_ports
else:
    system.mem_ctrl = MemCtrl(); system.mem_ctrl.dram = globals()[args.mem](); system.mem_ctrl.dram.range = system.mem_ranges[0]; system.mem_ctrl.port = system.membus.mem_side_ports
system.workload = SEWorkload.init_compatible(args.cmd)
process = Process(); process.cmd = [args.cmd] + args.options.split()
system.cpu.workload = process; system.cpu.createThreads()
root = Root(full_system=False, system=system)
m5.instantiate(); ev = m5.simulate()
print("gem5 exit @ tick %d: %s" % (m5.curTick(), ev.getCause()))
