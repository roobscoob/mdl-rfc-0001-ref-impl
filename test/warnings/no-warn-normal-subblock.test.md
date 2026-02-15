---
description = "Normal sub-block call produces no warnings"
expect_output = "10"
expect_warnings = []
---
# Main
1. x = 10
2. [](#Child)

## Child
1. **{x}**
