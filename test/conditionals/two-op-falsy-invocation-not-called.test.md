---
description = "Two-operand falsy does not invoke block"
expect_output = "after"
---
# Main
1. false ? [](#NeverRun)
2. **after**

## NeverRun
1. **should not print**