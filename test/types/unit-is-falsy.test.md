---
description = "Unit is falsy in conditional"
expect_output = "side\nfalsy"
---
# Main
1. u = **{"side"}**
2. **{u ? "truthy" : "falsy"}**
