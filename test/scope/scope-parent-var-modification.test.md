---
description = "Sub-block modifies parent variable"
expect_output = "99"
---
# Main
1. x = 1
2. [](#Modify)
3. **{x}**

## Modify
1. x = 99
