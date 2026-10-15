# Measured synthetic search results

| Method | Median best score | Best to worst score | Exact grid optimum hits |
| --- | ---: | ---: | ---: |
| Annealing | 2.565 | 2.565 to 2.565 | 30/30 |
| Greedy | 2.565 | 2.565 to 2.565 | 30/30 |
| Random | 4.220 | 2.565 to 8.425 | 5/30 |
| Coarse grid | 3.480 | 3.480 to 3.480 | No (deterministic) |

Each method receives 256 objective calls. Stochastic rows summarize seeds 1 through 30. The grid result is deterministic and repeated for CSV uniformity. Ranges are observed minima and maxima, not confidence intervals.

| Method | Median gap to oracle | Median shifted score | Median accepted uphill moves |
| --- | ---: | ---: | ---: |
| Annealing | 0.000 | 5.917 | 27.0 |
| Greedy | 0.000 | 5.917 | 0.0 |
| Random | 1.655 | 7.407 | 0.0 |
| Coarse grid | 0.915 | 7.081 | 0.0 |

The shifted score averages four combinations of coordinate offsets of plus or minus 0.1. It is calculated only after candidate selection. It is a deterministic sensitivity check.
