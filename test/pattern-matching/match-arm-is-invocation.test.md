---
description = "Match arm result is block invocation"
expect_output = "from block"
---
# Main
1. x = match 1
    - 1: [](#GetStr)
    - otherwise: "fallback"
2. **{x}**

## GetStr
1. "from block"
