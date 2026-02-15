---
description = "Deep scope chain with variable at each level"
expect_output = "15"
---
# Main
1. a = 1
2. [](#L2)

## L2
1. b = 2
2. [](#L3)

### L3
1. c = 3
2. [](#L4)

#### L4
1. d = 4
2. [](#L5)

##### L5
1. e = 5
2. **{a + b + c + d + e}**
