---
description = "Non-contiguous indices still execute sequentially"
expect_output = "15"
---
# Main
1. x = 5
5. y = 10
10. **{x + y}**
