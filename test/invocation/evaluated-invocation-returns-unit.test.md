---
description = "Evaluated invocation of print returns Unit"
expect_output = "side effect\n()"
---
# Main
1. x = ![](#Sub)
2. **{x}**

## Sub
**{"side effect"}**
