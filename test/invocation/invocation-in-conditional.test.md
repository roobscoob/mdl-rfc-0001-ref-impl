---
description = "Conditional branches invoke different blocks"
expect_output = "big"
---
# Main
1. x = 10
2. **{x > 5 ? [](#Big) : [](#Small)}**

## Big
1. "big"

## Small
1. "small"
