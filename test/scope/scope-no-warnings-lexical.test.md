---
description = "Normal sub-block call produces no scope warnings"
expect_warnings = []
---
# Main
1. x = 10
2. [](#Child)

## Child
1. **{x}**
