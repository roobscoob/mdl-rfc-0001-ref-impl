---
description = "Each nesting level adds a variable, deepest reads all"
expect_output = "6"
---
# Main
1. a = 1
2. [](#L2)

## L2
1. b = 2
2. [](#L3)

### L3
1. c = 3
2. **{a + b + c}**
