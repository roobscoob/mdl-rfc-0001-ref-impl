---
description = "Match with unit value goes to otherwise"
expect_output = "side\nwas unit"
---
# Main
1. u = **{"side"}**
2. result = match u
    - 1: "number"
    - otherwise: "was unit"
3. **{result}**
