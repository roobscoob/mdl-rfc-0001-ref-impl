---
description = "Mutual recursion between two sub-blocks"
expect_output = "even"
---
# Main
1. **{[4](#IsEven)}**

## IsEven
1. #0 == 0 ? "even" : [#0 - 1](#IsOdd)

## IsOdd
1. #0 == 0 ? "odd" : [#0 - 1](#IsEven)
