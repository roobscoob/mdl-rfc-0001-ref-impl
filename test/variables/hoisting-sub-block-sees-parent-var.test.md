---
description = "Sub-block reads parent variable via scope inheritance"
expect_output = "10"
---
# Main
1. x = 10
2. [](#Child)

## Child
1. **{x}**
