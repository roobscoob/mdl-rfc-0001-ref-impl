---
description = "Sub-blocks with distinct names under different top-level blocks"
expect_output = "from A"
---
# Main
1. **{[](#WorkerA)}**

## WorkerA
1. "from A"

# Other
## WorkerB
1. "from B"
