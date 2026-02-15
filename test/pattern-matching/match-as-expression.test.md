---
description = "Match used as expression in assignment"
expect_output = "10"
---
# Main
1. x = 5
2. y = match x
    - 5: 10
    - otherwise: 0
3. **{y}**
