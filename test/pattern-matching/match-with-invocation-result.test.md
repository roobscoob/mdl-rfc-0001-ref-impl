---
description = "Match on block invocation result"
expect_output = "big"
---
# Main
1. x = match [](#GetVal)
    - 1: "small"
    - 10: "big"
    - otherwise: "unknown"
2. **{x}**

## GetVal
1. 10
