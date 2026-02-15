---
description = "Match dispatches to different blocks"
expect_output = "10"
---
# Main
1. op = "double"
2. result = match op
    - "double": [5](#Double)
    - "triple": [5](#Triple)
    - otherwise: 0
3. **{result}**

## Double
1. #0 * 2

## Triple
1. #0 * 3
