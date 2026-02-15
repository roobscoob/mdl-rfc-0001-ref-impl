---
description = "Match on conditional result"
expect_output = "got yes"
---
# Main
1. val = true ? "yes"
2. result = match val
    - "yes": "got yes"
    - otherwise n: "got other"
3. **{result}**
