---
description = "Sibling sub-blocks do not share locally defined variables"
expect_output = "10"
---
# Main
1. x = 10
2. [](#A)
3. [](#B)

## A
1. y = 20

## B
1. **{x}**
