---
description = "Exact arm match is found"
expect_output = "matched"
---
# Main
1. x = match "target"
    - "target": "matched"
    - "other": "wrong"
    - otherwise: "fallback"
2. **{x}**
