---
description = "Conditional branches invoke different blocks"
expect_output = "yes block"
---
# Main
1. **{true ? [](#Yes) : [](#No)}**

## Yes
1. "yes block"

## No
1. "no block"
