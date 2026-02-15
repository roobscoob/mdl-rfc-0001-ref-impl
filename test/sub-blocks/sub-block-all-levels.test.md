---
description = "All 6 levels with scope inheritance through each"
expect_output = "21"
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
2. [](#L6)

###### L6
1. f = 6
2. **{a + b + c + d + e + f}**
