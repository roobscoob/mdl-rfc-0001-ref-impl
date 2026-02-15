# Main
1. x = #0
2. x > 0 ? **{"positive"}**
3. x > 100 ? **{"big!"}**
4. label = x > 0 ? "pos" : "non-pos"
5. **{label}**

## Classify
1. n = #0
2. n > 0 ? [](#Pos) : n < 0 ? [](#Neg) : [](#Zero)

### Pos
1. **{"positive"}**

### Neg
1. **{"negative"}**

### Zero
1. **{"zero"}**
