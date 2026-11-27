# Licence

Copyright (c) 2026 Mohamed El-Hadedy.

This repository is released under reciprocal licences. Anyone may use, study and build on the work. Anyone who distributes a modified
version must publish it under the same terms.

| Part | Folders | Licence | Text |
|---|---|---|---|
| Hardware: RTL, build scripts, bitstreams | `rtl/`, `synthesis/`, `bitstreams/` | CERN Open Hardware Licence Version 2, Strongly Reciprocal | `LICENSES/CERN-OHL-S-2.0.txt` |
| Software: board programs, checks, simulator, figure scripts | `software/`, `model/`, `docs/figure_scripts/` | GNU General Public License, version 3 or later | `LICENSES/GPL-3.0-or-later.txt` |
| Results, figures and documentation | `results/`, `docs/img/`, `README.md` | Creative Commons Attribution 4.0 International | `LICENSES/CC-BY-4.0.txt` |

Every source file carries its SPDX identifier in its first lines. Where a file has no identifier, the table above decides by folder.

For the CERN-OHL-S: the Source Location of this work is https://github.com/mealycpp/ruty-atom.

Third-party material is not included. The software comparator in `software/xkcp_modes_bench.c` builds against the Keccak team's XKCP
(commit fa7033a212c831249b7e17e785ee1a4e24fc04c4), which must be fetched separately and keeps its own licence.
