---
description = "Sub-block shadows parent variable locally"
expect_output = "1"
---
# Main
1. x = 1
2. [](#Modify)
3. **{x}**

## Modify
1. x = 99
