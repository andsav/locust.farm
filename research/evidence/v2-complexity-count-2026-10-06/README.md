# Evidence for the v2 complexity count

Raw results behind [how much v2 adds](../../v2-complexity-count-2026-10-06.md),
collected on 6 October 2026.

- `ledgers.json`: what the eight readers returned, unedited apart from
  shortened paths. `baseline` is the inventory of the code as it stood.
  `ledgers` holds one list per piece of the plan; every item names its
  measure, whether it is added, removed or changed, and the plan or code
  location it rests on. `hidden` is the reader who looked for what the count
  missed. `cuts` is the reader who looked for what could go.
- `count.workflow.js`: the instructions each reader was given. It is the
  method. Paths that begin `scratch/` pointed at a session folder that is not
  kept; the host safety text they refer to is now
  [the host safety and ending plan](../../../docs/host-safety-and-ending-plan.md).

Nothing was built or run. The readers worked while other sessions were
changing the code, so the baseline gives two snapshots: the working tree at
the time, 103,092 lines of Rust, and commit `43f03b4`, 101,443 lines. The
plans were at `ae85baf`.

The line figures cannot be reproduced by a script. Each is a reader's
estimate from the files a phase names and their present size. The item
counts can be recomputed from `ledgers.json`.
