# Shrink to the comfort floor

Just over the portrait width, landing at exactly 9.0pt — the `>=` boundary of `table_comfort_pt`, which is why this fixture is sized as it is; one step lower and it reflows instead. Called a *slight* shrink until T30 raised the base to 12pt: the size did not move, because a table fits at the size it fits at whatever the base, but 12 → 9 is a quarter off rather than a tenth and "slight" stopped being true of it.

| Column 0 | Column 1 | Column 2 | Column 3 | Column 4 |
|---|---|---|---|---|
| xxxxxxxxxxxxxxxxxxxxx | xxxxxxxxxxxxxxxxxxxxx | xxxxxxxxxxxxxxxxxxxxx | xxxxxxxxxxxxxxxxxxxxx | xxxxxxxxxxxxxxxxxxxxx |
| xxxxxxxxxxxxxxxxxxxxx | xxxxxxxxxxxxxxxxxxxxx | xxxxxxxxxxxxxxxxxxxxx | xxxxxxxxxxxxxxxxxxxxx | xxxxxxxxxxxxxxxxxxxxx |
| xxxxxxxxxxxxxxxxxxxxx | xxxxxxxxxxxxxxxxxxxxx | xxxxxxxxxxxxxxxxxxxxx | xxxxxxxxxxxxxxxxxxxxx | xxxxxxxxxxxxxxxxxxxxx |
