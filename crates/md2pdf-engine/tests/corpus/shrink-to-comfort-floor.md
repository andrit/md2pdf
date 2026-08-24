# Shrink to the comfort floor

Just over the portrait width, landing at exactly 10.0pt — the `>=` boundary of `table_comfort_pt`, which is why this fixture is sized as it is; one step lower and it reflows instead, and since it is the only fixture that shrinks, the census loses sight of that whole rung if it drifts. Re-sized twice, neither time because the ladder changed: T30 raised the base to 12pt and the *name* stopped being true, then T26c moved the floor itself to 10.0 by eye, so the cells lost two characters each to land back on the boundary. The size is a property of the content and the page; what moves is which side of the floor it falls on.

| Column 0 | Column 1 | Column 2 | Column 3 | Column 4 |
|---|---|---|---|---|
| xxxxxxxxxxxxxxxxxxx | xxxxxxxxxxxxxxxxxxx | xxxxxxxxxxxxxxxxxxx | xxxxxxxxxxxxxxxxxxx | xxxxxxxxxxxxxxxxxxx |
| xxxxxxxxxxxxxxxxxxx | xxxxxxxxxxxxxxxxxxx | xxxxxxxxxxxxxxxxxxx | xxxxxxxxxxxxxxxxxxx | xxxxxxxxxxxxxxxxxxx |
| xxxxxxxxxxxxxxxxxxx | xxxxxxxxxxxxxxxxxxx | xxxxxxxxxxxxxxxxxxx | xxxxxxxxxxxxxxxxxxx | xxxxxxxxxxxxxxxxxxx |
