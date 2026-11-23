## The compute programs (21)

### Instructions executed, by group (percent of the program's instructions)

| program | instructions | sites | moves and constants | arithmetic | comparison and logic | control | step marks | data | calls and returns | closures and cells | everything else |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| closures | 2,100,021 | 42 | 28.6 | 9.5 | 0.0 | 4.8 | 23.8 | 0.0 | 19.0 | 14.3 | 0.0 |
| csv | 440,048 | 69 | 22.7 | 13.6 | 0.0 | 13.6 | 18.2 | 22.7 | 9.1 | 0.0 | 0.0 |
| deltablue | 176,804 | 104 | 0.9 | 37.6 | 5.4 | 6.0 | 33.7 | 16.4 | 0.0 | 0.0 | 0.0 |
| dispatch | 2,450,042 | 63 | 18.4 | 32.7 | 0.0 | 6.1 | 18.4 | 12.2 | 12.2 | 0.0 | 0.0 |
| fannkuch | 12,416,903 | 181 | 5.4 | 11.8 | 10.1 | 10.3 | 42.9 | 19.2 | 0.3 | 0.0 | 0.0 |
| fib | 2,185,075 | 29 | 11.1 | 16.7 | 11.1 | 5.6 | 22.2 | 0.0 | 22.2 | 11.1 | 0.0 |
| json | 440,038 | 60 | 31.8 | 4.5 | 4.5 | 4.5 | 9.1 | 40.9 | 4.5 | 0.0 | 0.0 |
| json-deep | 44,855 | 129 | 26.1 | 5.7 | 2.4 | 12.2 | 24.4 | 9.8 | 14.6 | 4.9 | 0.0 |
| lists | 660,054 | 62 | 9.1 | 31.8 | 9.1 | 9.1 | 18.2 | 0.0 | 22.7 | 0.0 | 0.0 |
| loop | 2,400,017 | 23 | 0.0 | 50.0 | 0.0 | 16.7 | 33.3 | 0.0 | 0.0 | 0.0 | 0.0 |
| mandel | 15,729,301 | 86 | 4.7 | 44.8 | 12.2 | 12.6 | 25.7 | 0.0 | 0.0 | 0.0 | 0.0 |
| maps | 900,030 | 49 | 27.8 | 8.3 | 0.0 | 16.7 | 25.0 | 16.7 | 5.6 | 0.0 | 0.0 |
| nbody | 17,881,222 | 450 | 3.9 | 35.2 | 0.0 | 3.8 | 20.4 | 33.6 | 1.3 | 1.8 | 0.0 |
| regex-ish | 720,066 | 75 | 44.4 | 0.0 | 0.0 | 11.1 | 22.2 | 11.1 | 11.1 | 0.0 | 0.0 |
| richards | 760,143 | 138 | 6.6 | 9.2 | 6.6 | 7.9 | 25.0 | 44.7 | 0.0 | 0.0 | 0.0 |
| sieve | 7,612,450 | 81 | 7.9 | 8.3 | 8.0 | 23.8 | 40.1 | 8.0 | 3.9 | 0.0 | 0.0 |
| spectral | 8,458,608 | 193 | 14.5 | 47.3 | 0.0 | 4.8 | 14.4 | 4.7 | 9.5 | 4.7 | 0.0 |
| strbuild | 40,033 | 37 | 0.0 | 25.0 | 0.0 | 25.0 | 50.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| strings | 480,027 | 35 | 37.5 | 0.0 | 0.0 | 12.5 | 25.0 | 12.5 | 12.5 | 0.0 | 0.0 |
| trees | 16,878,487 | 130 | 24.1 | 8.0 | 8.0 | 0.0 | 16.0 | 19.9 | 16.0 | 7.9 | 0.0 |
| words | 929,153 | 100 | 21.5 | 12.8 | 0.3 | 22.0 | 21.7 | 13.0 | 8.6 | 0.0 | 0.0 |

### The script's top level (function 0)

What runs once a process before anything else can: the statements outside every function, which declare the script's functions and build its tables.

| program | instructions | in the top level | of the run (%) | its busiest instructions |
|---|---:|---:|---:|---:|
| closures | 2,100,021 | 1,100,021 | 52.4 | `mov` 400,004, `steps` 300,004, `call` 200,000, `add` 100,000, `forloop` 100,000, `loadint` 3 |
| csv | 440,048 | 440,048 | 100.0 | `steps` 80,010, `loadk` 60,007, `mov` 40,009, `insertmap` 40,004, `newmap` 20,002, `interp` 20,001 |
| deltablue | 176,804 | 176,804 | 100.0 | `steps` 59,594, `getindex` 19,000, `mulk` 19,000, `addk` 19,000, `forloop` 10,020, `setindex` 10,000 |
| dispatch | 2,450,042 | 2,100,042 | 85.7 | `steps` 450,006, `mov` 450,005, `modk` 450,000, `getindex` 300,000, `call` 150,000, `addk` 150,000 |
| fannkuch | 12,416,903 | 12,416,903 | 100.0 | `steps` 5,324,296, `getindex` 1,321,062, `setindex` 1,060,163, `addk` 860,250, `loop` 819,939, `ltj` 750,657 |
| fib | 2,185,075 | 14 | 0.0 | `mov` 3, `cellnew` 1, `loadnull` 1, `closure` 1, `steps` 1, `loadint` 1 |
| json | 440,038 | 440,038 | 100.0 | `loadk` 100,005, `insertmap` 80,003, `mov` 40,011, `steps` 40,006, `appendlist` 40,000, `callm` 20,002 |
| json-deep | 44,855 | 67 | 0.1 | `mov` 17, `steps` 10, `loadk` 7, `loadint` 6, `insertmap` 6, `callb` 3 |
| lists | 660,054 | 420,050 | 63.6 | `steps` 120,007, `mov` 60,014, `callm` 60,005, `mulk` 60,000, `modk` 60,000, `forloop` 60,000 |
| loop | 2,400,017 | 2,400,017 | 100.0 | `steps` 800,004, `mul` 400,000, `modk` 400,000, `add` 400,000, `forloop` 400,000, `loadint` 3 |
| mandel | 15,729,301 | 15,729,301 | 100.0 | `steps` 4,037,556, `mul` 3,806,510, `add` 1,877,655, `testn` 1,302,970, `ltk` 651,485, `not` 641,279 |
| maps | 900,030 | 900,030 | 100.0 | `steps` 225,007, `mov` 150,006, `loadk` 100,004, `interp` 100,000, `forloop` 100,000, `setindex` 50,000 |
| nbody | 17,881,222 | 120,324 | 0.7 | `steps` 40,036, `loadk` 20,084, `mov` 20,016, `call` 20,002, `forloop` 20,000, `insertmap` 44 |
| regex-ish | 720,066 | 720,066 | 100.0 | `mov` 160,018, `loadk` 160,013, `steps` 160,009, `callm` 80,007, `interp` 80,002, `forloop` 80,000 |
| richards | 760,143 | 760,143 | 100.0 | `steps` 190,031, `getindex` 169,998, `getfield` 130,004, `forloop` 50,010, `lej` 50,000, `addk` 40,002 |
| sieve | 7,612,450 | 7,612,450 | 100.0 | `steps` 3,051,531, `loop` 908,372, `lej` 608,473, `setindex` 607,827, `add` 607,825, `testn` 300,547 |
| spectral | 8,458,608 | 2,088 | 0.0 | `steps` 542, `getindex` 400, `mov` 253, `forloop` 210, `mul` 200, `add` 200 |
| strbuild | 40,033 | 40,033 | 100.0 | `steps` 20,006, `addk` 10,000, `forloop` 10,000, `mov` 6, `loadk` 5, `loadint` 4 |
| strings | 480,027 | 480,027 | 100.0 | `mov` 120,007, `steps` 120,005, `loadk` 60,004, `callm` 60,002, `interp` 60,000, `forloop` 60,000 |
| trees | 16,878,487 | 54,743 | 0.3 | `mov` 21,868, `steps` 10,954, `call` 10,916, `add` 5,456, `forloop` 5,456, `loadint` 19 |
| words | 929,153 | 929,153 | 100.0 | `steps` 202,008, `mov` 160,015, `callm` 80,004, `testn` 41,993, `iternext` 40,999, `getindex` 40,997 |

### Arithmetic: the operand types seen (percent of the group's executions)

| program | executed | int, int | float, float | int and float | str, str | other |
|---|---:|---:|---:|---:|---:|---:|
| closures | 200,000 | 100.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| csv | 60,000 | 33.3 | 33.3 | 33.3 | 0.0 | 0.0 |
| deltablue | 66,541 | 100.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| dispatch | 800,000 | 100.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| fannkuch | 1,459,617 | 100.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| fib | 364,176 | 100.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| json | 20,000 | 100.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| json-deep | 2,549 | 100.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| lists | 210,003 | 100.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| loop | 1,200,000 | 100.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| mandel | 7,048,527 | 9.0 | 89.5 | 1.5 | 0.0 | 0.0 |
| maps | 75,000 | 100.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| nbody | 6,300,361 | 1.6 | 98.4 | 0.0 | 0.0 | 0.0 |
| regex-ish | 1 | 100.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| richards | 70,012 | 100.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| sieve | 635,016 | 100.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| spectral | 4,000,401 | 70.0 | 20.0 | 10.0 | 0.0 | 0.0 |
| strbuild | 10,001 | 0.0 | 0.0 | 0.0 | 100.0 | 0.0 |
| strings | 0 | - | - | - | - | - |
| trees | 1,343,511 | 100.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| words | 119,003 | 100.0 | 0.0 | 0.0 | 0.0 | 0.0 |

### Comparisons: the operand types seen (percent of the group's executions)

| program | executed | int, int | float, float | int and float | str, str | other |
|---|---:|---:|---:|---:|---:|---:|
| closures | 0 | - | - | - | - | - |
| csv | 0 | - | - | - | - | - |
| deltablue | 9,500 | 100.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| dispatch | 0 | - | - | - | - | - |
| fannkuch | 1,259,719 | 100.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| fib | 242,785 | 100.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| json | 20,000 | 100.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| json-deep | 1,093 | 100.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| lists | 60,000 | 100.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| loop | 0 | - | - | - | - | - |
| mandel | 1,277,370 | 51.0 | 49.0 | 0.0 | 0.0 | 0.0 |
| maps | 0 | - | - | - | - | - |
| nbody | 0 | - | - | - | - | - |
| regex-ish | 0 | - | - | - | - | - |
| richards | 50,000 | 100.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| sieve | 608,473 | 100.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| spectral | 0 | - | - | - | - | - |
| strbuild | 0 | - | - | - | - | - |
| strings | 0 | - | - | - | - | - |
| trees | 1,348,962 | 50.0 | 0.0 | 0.0 | 0.0 | 50.0 |
| words | 2,112 | 94.4 | 0.0 | 0.0 | 5.6 | 0.0 |

### How settled a site's types are

Arithmetic and comparison sites. "One kind" is a site that saw one pair of operand types for the whole run.

| program | sites | sites with one kind | executions | at a site with one kind (%) |
|---|---:|---:|---:|---:|
| closures | 2 | 2 | 200,000 | 100.0 |
| csv | 3 | 3 | 60,000 | 100.0 |
| deltablue | 11 | 11 | 76,041 | 100.0 |
| dispatch | 8 | 8 | 800,000 | 100.0 |
| fannkuch | 21 | 21 | 2,719,336 | 100.0 |
| fib | 4 | 4 | 606,961 | 100.0 |
| json | 2 | 2 | 40,000 | 100.0 |
| json-deep | 5 | 5 | 3,642 | 100.0 |
| lists | 6 | 6 | 270,003 | 100.0 |
| loop | 3 | 3 | 1,200,000 | 100.0 |
| mandel | 20 | 20 | 8,325,897 | 100.0 |
| maps | 2 | 2 | 75,000 | 100.0 |
| nbody | 82 | 82 | 6,300,361 | 100.0 |
| regex-ish | 1 | 1 | 1 | 100.0 |
| richards | 14 | 14 | 120,012 | 100.0 |
| sieve | 7 | 7 | 1,243,489 | 100.0 |
| spectral | 17 | 17 | 4,000,401 | 100.0 |
| strbuild | 2 | 2 | 10,001 | 100.0 |
| strings | 0 | 0 | 0 | - |
| trees | 11 | 10 | 2,692,473 | 74.9 |
| words | 6 | 6 | 121,115 | 100.0 |

### Field reads and writes (`getfield`, `setfield`)

A shape is a record's keys in order (a map of up to 16 keys); a larger map is a dictionary. "One position" is a site where the key sat at one index for the whole run, which is what the VM's cache cell holds today.

| program | executed | sites | sites with one shape | one shape (%) | two (%) | 3 to 8 (%) | more than 8 (%) | one position (%) | on a dictionary (%) | not on a map (%) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| closures | 0 | 0 | 0 | - | - | - | - | - | - | - |
| csv | 20,000 | 1 | 1 | 100.0 | 0.0 | 0.0 | 0.0 | 100.0 | 0.0 | 0.0 |
| deltablue | 0 | 0 | 0 | - | - | - | - | - | - | - |
| dispatch | 0 | 0 | 0 | - | - | - | - | - | - | - |
| fannkuch | 0 | 0 | 0 | - | - | - | - | - | - | - |
| fib | 0 | 0 | 0 | - | - | - | - | - | - | - |
| json | 0 | 0 | 0 | - | - | - | - | - | - | - |
| json-deep | 1,457 | 3 | 3 | 100.0 | 0.0 | 0.0 | 0.0 | 100.0 | 0.0 | 0.0 |
| lists | 0 | 0 | 0 | - | - | - | - | - | - | - |
| loop | 0 | 0 | 0 | - | - | - | - | - | - | - |
| mandel | 0 | 0 | 0 | - | - | - | - | - | - | - |
| maps | 0 | 0 | 0 | - | - | - | - | - | - | - |
| nbody | 5,700,263 | 57 | 57 | 100.0 | 0.0 | 0.0 | 0.0 | 100.0 | 0.0 | 0.0 |
| regex-ish | 0 | 0 | 0 | - | - | - | - | - | - | - |
| richards | 169,998 | 13 | 13 | 100.0 | 0.0 | 0.0 | 0.0 | 100.0 | 0.0 | 0.0 |
| sieve | 0 | 0 | 0 | - | - | - | - | - | - | - |
| spectral | 0 | 0 | 0 | - | - | - | - | - | - | - |
| strbuild | 0 | 0 | 0 | - | - | - | - | - | - | - |
| strings | 0 | 0 | 0 | - | - | - | - | - | - | - |
| trees | 0 | 0 | 0 | - | - | - | - | - | - | - |
| words | 0 | 0 | 0 | - | - | - | - | - | - | - |

### Indexing (`getindex`, `setindex`): what was indexed by what (percent of executions)

| program | executed | at a site with one kind (%) | list[int] | map[str] |
|---|---:|---:|---:|---:|
| closures | 0 | - | - | - |
| csv | 0 | - | - | - |
| deltablue | 29,000 | 100.0 | 100.0 | 0.0 |
| dispatch | 300,000 | 100.0 | 50.0 | 50.0 |
| fannkuch | 2,381,225 | 100.0 | 100.0 | 0.0 |
| fib | 0 | - | - | - |
| json | 0 | - | - | - |
| json-deep | 0 | - | - | - |
| lists | 2 | 100.0 | 100.0 | 0.0 |
| loop | 0 | - | - | - |
| mandel | 0 | - | - | - |
| maps | 50,000 | 100.0 | 0.0 | 100.0 |
| nbody | 300,033 | 100.0 | 100.0 | 0.0 |
| regex-ish | 0 | - | - | - |
| richards | 169,998 | 100.0 | 100.0 | 0.0 |
| sieve | 608,373 | 100.0 | 100.0 | 0.0 |
| spectral | 400,400 | 100.0 | 100.0 | 0.0 |
| strbuild | 0 | - | - | - |
| strings | 0 | - | - | - |
| trees | 1,343,498 | 100.0 | 100.0 | 0.0 |
| words | 80,997 | 100.0 | 0.0 | 100.0 |

### Calls

`call` and `tailcall` call a value; `callb` calls a builtin the lowering resolved; `callm` calls a method on a value.

| program | `call` | sites | sites with one callee | at a site with one callee (%) | to a function of the script (%) | to a builtin value (%) | `callb` | `callm` | `callm` at a site with one receiver type (%) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| closures | 200,000 | 2 | 2 | 100.0 | 100.0 | 0.0 | 0 | 0 | - |
| csv | 0 | 0 | 0 | - | - | - | 20,001 | 20,001 | 100.0 |
| deltablue | 0 | 0 | 0 | - | - | - | 1 | 21 | 100.0 |
| dispatch | 150,000 | 1 | 0 | 0.0 | 100.0 | 0.0 | 0 | 0 | - |
| fannkuch | 0 | 0 | 0 | - | - | - | 0 | 40,336 | 100.0 |
| fib | 242,785 | 3 | 3 | 100.0 | 100.0 | 0.0 | 0 | 0 | - |
| json | 0 | 0 | 0 | - | - | - | 2 | 20,002 | 100.0 |
| json-deep | 2,186 | 4 | 4 | 100.0 | 100.0 | 0.0 | 3 | 2,187 | 100.0 |
| lists | 0 | 0 | 0 | - | - | - | 0 | 60,005 | 100.0 |
| loop | 0 | 0 | 0 | - | - | - | 0 | 0 | - |
| mandel | 0 | 0 | 0 | - | - | - | 0 | 0 | - |
| maps | 0 | 0 | 0 | - | - | - | 0 | 50,000 | 100.0 |
| nbody | 20,002 | 3 | 3 | 100.0 | 100.0 | 0.0 | 200,022 | 0 | - |
| regex-ish | 0 | 0 | 0 | - | - | - | 1 | 80,007 | 100.0 |
| richards | 0 | 0 | 0 | - | - | - | 1 | 6 | 100.0 |
| sieve | 0 | 0 | 0 | - | - | - | 0 | 300,001 | 100.0 |
| spectral | 400,040 | 6 | 6 | 100.0 | 100.0 | 0.0 | 2 | 4,100 | 100.0 |
| strbuild | 0 | 0 | 0 | - | - | - | 1 | 2 | 100.0 |
| strings | 0 | 0 | 0 | - | - | - | 0 | 60,002 | 100.0 |
| trees | 1,348,956 | 10 | 10 | 100.0 | 100.0 | 0.0 | 5 | 0 | - |
| words | 0 | 0 | 0 | - | - | - | 0 | 80,004 | 100.0 |

### Functions of the script: rows of argument types

For each function called through `call`: how many distinct rows of argument types it was called with (up to 16 are kept).

| program | functions called | called with one row | calls | calls to a function with one row (%) | most rows at one function |
|---|---:|---:|---:|---:|---:|
| closures | 2 | 2 | 200,000 | 100.0 | 1 |
| csv | 0 | 0 | 0 | - | 0 |
| deltablue | 0 | 0 | 0 | - | 0 |
| dispatch | 3 | 3 | 150,000 | 100.0 | 1 |
| fannkuch | 0 | 0 | 0 | - | 0 |
| fib | 1 | 1 | 242,785 | 100.0 | 1 |
| json | 0 | 0 | 0 | - | 0 |
| json-deep | 2 | 2 | 2,186 | 100.0 | 1 |
| lists | 0 | 0 | 0 | - | 0 |
| loop | 0 | 0 | 0 | - | 0 |
| mandel | 0 | 0 | 0 | - | 0 |
| maps | 0 | 0 | 0 | - | 0 |
| nbody | 2 | 2 | 20,002 | 100.0 | 1 |
| regex-ish | 0 | 0 | 0 | - | 0 |
| richards | 0 | 0 | 0 | - | 0 |
| sieve | 0 | 0 | 0 | - | 0 |
| spectral | 3 | 3 | 400,040 | 100.0 | 1 |
| strbuild | 0 | 0 | 0 | - | 0 |
| strings | 0 | 0 | 0 | - | 0 |
| trees | 2 | 2 | 1,348,956 | 100.0 | 1 |
| words | 0 | 0 | 0 | - | 0 |

### Where the work is

The module is the script as lowered: every function it holds and every instruction in them. A site executed is an instruction that ran at least once.

| program | functions in the module | functions entered | instructions in the module | sites executed | of the module (%) | executions a site | ten busiest sites (%) | busiest tenth of the sites (%) | sites that make 90% of the instructions |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| closures | 3 | 3 | 42 | 42 | 100.0 | 50000.5 | 47.6 | 19.0 | 19 |
| csv | 1 | 1 | 69 | 69 | 100.0 | 6377.5 | 45.4 | 27.3 | 20 |
| deltablue | 1 | 1 | 110 | 104 | 94.5 | 1700.0 | 53.7 | 53.7 | 17 |
| dispatch | 4 | 4 | 65 | 63 | 96.9 | 38889.6 | 61.2 | 36.7 | 17 |
| fannkuch | 1 | 1 | 201 | 181 | 90.0 | 68601.7 | 33.0 | 51.7 | 68 |
| fib | 2 | 2 | 29 | 29 | 100.0 | 75347.4 | 72.2 | 22.2 | 14 |
| json | 1 | 1 | 60 | 60 | 100.0 | 7334.0 | 45.5 | 27.3 | 20 |
| json-deep | 3 | 3 | 129 | 129 | 100.0 | 347.7 | 25.2 | 30.0 | 50 |
| lists | 3 | 3 | 64 | 62 | 96.9 | 10646.0 | 90.9 | 54.5 | 10 |
| loop | 1 | 1 | 23 | 23 | 100.0 | 104348.6 | 100.0 | 33.3 | 6 |
| mandel | 1 | 1 | 86 | 86 | 100.0 | 182898.8 | 40.4 | 32.4 | 23 |
| maps | 1 | 1 | 51 | 49 | 96.1 | 18368.0 | 55.6 | 22.2 | 17 |
| nbody | 3 | 3 | 457 | 450 | 98.5 | 39736.0 | 11.2 | 50.3 | 91 |
| regex-ish | 1 | 1 | 75 | 75 | 100.0 | 9600.9 | 100.0 | 77.8 | 9 |
| richards | 1 | 1 | 151 | 138 | 91.4 | 5508.3 | 44.7 | 48.7 | 45 |
| sieve | 1 | 1 | 88 | 81 | 92.0 | 93980.9 | 67.7 | 59.8 | 16 |
| spectral | 4 | 4 | 199 | 193 | 97.0 | 43827.0 | 47.3 | 70.9 | 28 |
| strbuild | 1 | 1 | 37 | 37 | 100.0 | 1082.0 | 99.9 | 74.9 | 4 |
| strings | 1 | 1 | 35 | 35 | 100.0 | 13715.1 | 100.0 | 37.5 | 8 |
| trees | 3 | 3 | 133 | 130 | 97.7 | 129834.5 | 36.0 | 42.0 | 38 |
| words | 1 | 1 | 108 | 100 | 92.6 | 9291.5 | 43.1 | 43.1 | 21 |

### What was made and copied (per 1,000 instructions)

A `mov` of a string, list, map or function, and a `loadk` of a string, each copy a counted value: one reference-count increment, and a decrement when the register is overwritten.

| program | lists | maps | closures | interpolated strings | `mov` of a counted value | `loadk` of a string | `mov` of a counted value (% of `mov`) |
|---|---:|---:|---:|---:|---:|---:|---:|
| closures | 0.00 | 0.00 | 47.62 | 0.00 | 142.86 | 0.00 | 60.0 |
| csv | 0.00 | 45.45 | 0.00 | 45.45 | 45.46 | 136.36 | 50.0 |
| deltablue | 0.01 | 0.01 | 0.00 | 0.01 | 0.14 | 0.05 | 2.3 |
| dispatch | 0.00 | 0.00 | 0.00 | 0.00 | 61.22 | 0.00 | 33.3 |
| fannkuch | 0.00 | 0.00 | 0.00 | 0.00 | 6.50 | 0.00 | 30.9 |
| fib | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 66.7 |
| json | 45.45 | 45.45 | 0.00 | 45.45 | 45.47 | 227.26 | 50.0 |
| json-deep | 8.12 | 24.39 | 0.04 | 0.02 | 113.92 | 57.01 | 73.7 |
| lists | 0.00 | 0.00 | 0.00 | 0.00 | 90.92 | 0.01 | 100.0 |
| loop | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.0 |
| mandel | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.00 | 0.0 |
| maps | 0.00 | 0.00 | 0.00 | 111.11 | 55.55 | 111.11 | 33.3 |
| nbody | 0.00 | 0.00 | 0.00 | 0.00 | 2.24 | 0.00 | 8.3 |
| regex-ish | 0.00 | 0.00 | 0.00 | 111.10 | 111.12 | 222.22 | 50.0 |
| richards | 0.00 | 0.01 | 0.00 | 0.00 | 0.01 | 0.03 | 0.0 |
| sieve | 0.00 | 0.00 | 0.00 | 0.00 | 39.41 | 0.00 | 100.0 |
| spectral | 0.00 | 0.00 | 0.00 | 0.00 | 0.50 | 0.00 | 0.5 |
| strbuild | 0.00 | 0.02 | 0.00 | 0.00 | 0.05 | 0.12 | 33.3 |
| strings | 0.00 | 0.00 | 0.00 | 124.99 | 125.00 | 125.00 | 50.0 |
| trees | 39.96 | 0.00 | 0.00 | 0.00 | 0.97 | 0.00 | 74.9 |
| words | 0.00 | 0.00 | 0.00 | 43.05 | 129.16 | 43.06 | 75.0 |

### The builtins and methods called most

- **closures:** builtins: none. Methods: none.
- **csv:** builtins: `float` 20,000, `csv.read` 1. Methods: `.push()` 20,000, `.len()` 1.
- **deltablue:** builtins: `int` 1. Methods: `.push()` 20, `.get()` 1.
- **dispatch:** builtins: none. Methods: none.
- **fannkuch:** builtins: none. Methods: `.slice()` 40,320, `.push()` 16.
- **fib:** builtins: none. Methods: none.
- **json:** builtins: `json.stringify` 1, `json.parse` 1. Methods: `.push()` 20,000, `.len()` 2.
- **json-deep:** builtins: `int` 1, `json.stringify` 1, `json.parse` 1. Methods: `.has()` 1,093, `.push()` 1,092, `.get()` 1, `.len()` 1.
- **lists:** builtins: none. Methods: `.push()` 60,000, `.len()` 2, `.sort()` 1, `.filter()` 1, `.map()` 1.
- **loop:** builtins: none. Methods: none.
- **mandel:** builtins: none. Methods: none.
- **maps:** builtins: none. Methods: `.has()` 50,000.
- **nbody:** builtins: `math.sqrt` 200,020, `math.round` 2. Methods: none.
- **regex-ish:** builtins: `int` 1. Methods: `.push()` 80,000, `.len()` 2, `.get()` 1, `.join()` 1, `.replace()` 1.
- **richards:** builtins: `int` 1. Methods: `.push()` 5, `.get()` 1.
- **sieve:** builtins: none. Methods: `.push()` 300,001.
- **spectral:** builtins: `math.sqrt` 1, `math.round` 1. Methods: `.push()` 4,100.
- **strbuild:** builtins: `int` 1. Methods: `.get()` 1, `.len()` 1.
- **strings:** builtins: none. Methods: `.push()` 60,000, `.join()` 1, `.len()` 1.
- **trees:** builtins: `math.pow` 5. Methods: none.
- **words:** builtins: none. Methods: `.push()` 40,000, `.has()` 40,000, `.join()` 1, `.split()` 1, `.keys()` 1.

### Across the compute programs

- Step marks (`steps`) are 9.1% to 50.0% of a program's executed instructions, median 23.8%.
- Moves and constant loads are 0.0% to 44.4%, median 14.5%.
- Arithmetic and comparison executions at a site with one kind of operands: median 100.0%, lowest 74.9%.
- Arithmetic on two integers: median 100.0% of a program's arithmetic.
- `call` executions at a site with one callee: median 100.0%, lowest 0.0% (7 programs make such calls).
- Field reads and writes at a site with one shape: 5,891,718 of 5,891,718 executions in the 4 programs that have any.
- Calls to a function that was only ever called with one row of argument types: median 100.0%, lowest 100.0%.

## cigo's own script, one process a case

### Instructions executed, by group (percent of the program's instructions)

| program | instructions | sites | moves and constants | arithmetic | comparison and logic | control | step marks | data | calls and returns | closures and cells | everything else |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| cigo-audit-one | 66,764 | 37627 | 39.3 | 0.2 | 1.5 | 6.8 | 9.5 | 29.4 | 7.6 | 5.7 | 0.0 |
| cigo-guard-line | 76,522 | 36539 | 36.5 | 0.2 | 4.6 | 5.8 | 12.8 | 24.9 | 10.7 | 4.5 | 0.0 |
| cigo-help | 37,171 | 35217 | 45.4 | 0.2 | 0.1 | 0.3 | 1.3 | 47.6 | 0.4 | 4.5 | 0.2 |
| cigo-hook-benign | 76,981 | 36799 | 36.5 | 0.2 | 4.6 | 5.8 | 12.8 | 24.7 | 10.7 | 4.5 | 0.0 |
| cigo-hook-curl-sh | 80,290 | 37273 | 35.5 | 0.5 | 5.2 | 6.8 | 13.3 | 23.8 | 10.5 | 4.4 | 0.0 |
| cigo-list | 82,990 | 36301 | 39.4 | 0.6 | 2.4 | 6.4 | 9.5 | 24.6 | 11.4 | 5.6 | 0.1 |
| cigo-version | 36,933 | 34979 | 45.3 | 0.2 | 0.1 | 0.3 | 1.2 | 47.9 | 0.4 | 4.6 | 0.0 |

### The script's top level (function 0)

What runs once a process before anything else can: the statements outside every function, which declare the script's functions and build its tables.

| program | instructions | in the top level | of the run (%) | its busiest instructions |
|---|---:|---:|---:|---:|
| cigo-audit-one | 66,764 | 34,876 | 52.2 | `loadk` 13,893, `appendlist` 8,795, `insertmap` 4,517, `newlist` 2,323, `mov` 1,137, `loadnull` 938 |
| cigo-guard-line | 76,522 | 34,876 | 45.6 | `loadk` 13,893, `appendlist` 8,795, `insertmap` 4,517, `newlist` 2,323, `mov` 1,137, `loadnull` 938 |
| cigo-help | 37,171 | 34,841 | 93.7 | `loadk` 13,890, `appendlist` 8,795, `insertmap` 4,517, `newlist` 2,323, `mov` 1,132, `loadnull` 938 |
| cigo-hook-benign | 76,981 | 34,911 | 45.4 | `loadk` 13,896, `appendlist` 8,795, `insertmap` 4,517, `newlist` 2,323, `mov` 1,142, `loadnull` 938 |
| cigo-hook-curl-sh | 80,290 | 34,911 | 43.5 | `loadk` 13,896, `appendlist` 8,795, `insertmap` 4,517, `newlist` 2,323, `mov` 1,142, `loadnull` 938 |
| cigo-list | 82,990 | 34,841 | 42.0 | `loadk` 13,890, `appendlist` 8,795, `insertmap` 4,517, `newlist` 2,323, `mov` 1,132, `loadnull` 938 |
| cigo-version | 36,933 | 34,841 | 94.3 | `loadk` 13,890, `appendlist` 8,795, `insertmap` 4,517, `newlist` 2,323, `mov` 1,132, `loadnull` 938 |

### Arithmetic: the operand types seen (percent of the group's executions)

| program | executed | int, int | float, float | int and float | str, str | other |
|---|---:|---:|---:|---:|---:|---:|
| cigo-audit-one | 148 | 3.4 | 0.0 | 0.0 | 37.8 | 58.8 |
| cigo-guard-line | 161 | 18.0 | 0.0 | 0.0 | 32.9 | 49.1 |
| cigo-help | 81 | 0.0 | 0.0 | 0.0 | 3.7 | 96.3 |
| cigo-hook-benign | 175 | 16.6 | 0.0 | 0.0 | 38.3 | 45.1 |
| cigo-hook-curl-sh | 409 | 44.3 | 0.0 | 0.0 | 35.9 | 19.8 |
| cigo-list | 457 | 45.5 | 0.0 | 0.0 | 37.4 | 17.1 |
| cigo-version | 81 | 0.0 | 0.0 | 0.0 | 3.7 | 96.3 |

### Comparisons: the operand types seen (percent of the group's executions)

| program | executed | int, int | float, float | int and float | str, str | other |
|---|---:|---:|---:|---:|---:|---:|
| cigo-audit-one | 854 | 1.9 | 0.0 | 0.0 | 70.5 | 27.6 |
| cigo-guard-line | 3,446 | 0.7 | 0.0 | 0.0 | 98.7 | 0.6 |
| cigo-help | 23 | 0.0 | 0.0 | 0.0 | 87.0 | 13.0 |
| cigo-hook-benign | 3,474 | 0.8 | 0.0 | 0.0 | 98.5 | 0.7 |
| cigo-hook-curl-sh | 3,945 | 2.7 | 0.2 | 0.0 | 96.5 | 0.6 |
| cigo-list | 1,582 | 13.4 | 0.0 | 0.0 | 62.8 | 23.8 |
| cigo-version | 26 | 0.0 | 0.0 | 0.0 | 88.5 | 11.5 |

### How settled a site's types are

Arithmetic and comparison sites. "One kind" is a site that saw one pair of operand types for the whole run.

| program | sites | sites with one kind | executions | at a site with one kind (%) |
|---|---:|---:|---:|---:|
| cigo-audit-one | 177 | 175 | 1,002 | 78.3 |
| cigo-guard-line | 144 | 143 | 3,607 | 99.9 |
| cigo-help | 38 | 37 | 104 | 96.2 |
| cigo-hook-benign | 167 | 166 | 3,649 | 99.9 |
| cigo-hook-curl-sh | 194 | 193 | 4,354 | 99.9 |
| cigo-list | 116 | 114 | 2,039 | 89.6 |
| cigo-version | 41 | 40 | 107 | 96.3 |

### Field reads and writes (`getfield`, `setfield`)

A shape is a record's keys in order (a map of up to 16 keys); a larger map is a dictionary. "One position" is a site where the key sat at one index for the whole run, which is what the VM's cache cell holds today.

| program | executed | sites | sites with one shape | one shape (%) | two (%) | 3 to 8 (%) | more than 8 (%) | one position (%) | on a dictionary (%) | not on a map (%) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| cigo-audit-one | 422 | 52 | 44 | 19.7 | 80.3 | 0.0 | 0.0 | 72.3 | 1.4 | 0.0 |
| cigo-guard-line | 181 | 21 | 17 | 27.6 | 72.4 | 0.0 | 0.0 | 64.1 | 1.7 | 0.0 |
| cigo-help | 66 | 2 | 0 | 0.0 | 100.0 | 0.0 | 0.0 | 50.0 | 0.0 | 0.0 |
| cigo-hook-benign | 182 | 22 | 18 | 28.0 | 72.0 | 0.0 | 0.0 | 64.3 | 1.6 | 0.0 |
| cigo-hook-curl-sh | 198 | 38 | 34 | 33.8 | 66.2 | 0.0 | 0.0 | 67.2 | 1.5 | 0.0 |
| cigo-list | 541 | 18 | 9 | 20.5 | 79.5 | 0.0 | 0.0 | 84.3 | 0.0 | 0.0 |
| cigo-version | 66 | 2 | 0 | 0.0 | 100.0 | 0.0 | 0.0 | 50.0 | 0.0 | 0.0 |

### Indexing (`getindex`, `setindex`): what was indexed by what (percent of executions)

| program | executed | at a site with one kind (%) | map[str] | list[int] |
|---|---:|---:|---:|---:|
| cigo-audit-one | 878 | 100.0 | 99.2 | 0.8 |
| cigo-guard-line | 1,140 | 100.0 | 99.4 | 0.6 |
| cigo-help | 106 | 100.0 | 93.4 | 6.6 |
| cigo-hook-benign | 1,143 | 100.0 | 99.4 | 0.6 |
| cigo-hook-curl-sh | 1,144 | 100.0 | 99.3 | 0.7 |
| cigo-list | 1,296 | 100.0 | 82.8 | 17.2 |
| cigo-version | 106 | 100.0 | 93.4 | 6.6 |

### Calls

`call` and `tailcall` call a value; `callb` calls a builtin the lowering resolved; `callm` calls a method on a value.

| program | `call` | sites | sites with one callee | at a site with one callee (%) | to a function of the script (%) | to a builtin value (%) | `callb` | `callm` | `callm` at a site with one receiver type (%) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| cigo-audit-one | 893 | 125 | 125 | 100.0 | 100.0 | 0.0 | 1,325 | 1,829 | 100.0 |
| cigo-guard-line | 1,598 | 91 | 91 | 100.0 | 100.0 | 0.0 | 3,382 | 1,585 | 100.0 |
| cigo-help | 30 | 27 | 27 | 100.0 | 100.0 | 0.0 | 74 | 17 | 100.0 |
| cigo-hook-benign | 1,619 | 105 | 104 | 99.9 | 100.0 | 0.0 | 3,404 | 1,608 | 100.0 |
| cigo-hook-curl-sh | 1,633 | 119 | 118 | 99.9 | 100.0 | 0.0 | 3,421 | 1,708 | 100.0 |
| cigo-list | 1,192 | 63 | 63 | 100.0 | 100.0 | 0.0 | 2,046 | 4,019 | 100.0 |
| cigo-version | 32 | 29 | 29 | 100.0 | 100.0 | 0.0 | 76 | 18 | 100.0 |

### Functions of the script: rows of argument types

For each function called through `call`: how many distinct rows of argument types it was called with (up to 16 are kept).

| program | functions called | called with one row | calls | calls to a function with one row (%) | most rows at one function |
|---|---:|---:|---:|---:|---:|
| cigo-audit-one | 71 | 68 | 893 | 72.1 | 3 |
| cigo-guard-line | 50 | 47 | 1,598 | 4.9 | 3 |
| cigo-help | 11 | 9 | 30 | 50.0 | 2 |
| cigo-hook-benign | 57 | 54 | 1,619 | 6.2 | 3 |
| cigo-hook-curl-sh | 70 | 67 | 1,633 | 7.0 | 3 |
| cigo-list | 34 | 31 | 1,192 | 72.4 | 3 |
| cigo-version | 13 | 11 | 32 | 53.1 | 2 |

### Where the work is

The module is the script as lowered: every function it holds and every instruction in them. A site executed is an instruction that ran at least once.

| program | functions in the module | functions entered | instructions in the module | sites executed | of the module (%) | executions a site | ten busiest sites (%) | busiest tenth of the sites (%) | sites that make 90% of the instructions |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| cigo-audit-one | 1038 | 80 | 115,268 | 37,627 | 32.6 | 1.8 | 3.2 | 49.3 | 30,951 |
| cigo-guard-line | 1038 | 55 | 115,268 | 36,539 | 31.7 | 2.1 | 19.2 | 57.0 | 28,887 |
| cigo-help | 1038 | 13 | 115,268 | 35,217 | 30.6 | 1.1 | 0.9 | 14.7 | 31,500 |
| cigo-hook-benign | 1038 | 62 | 115,268 | 36,799 | 31.9 | 2.1 | 19.1 | 57.0 | 29,101 |
| cigo-hook-curl-sh | 1038 | 80 | 115,268 | 37,273 | 32.3 | 2.2 | 18.3 | 58.2 | 29,244 |
| cigo-list | 1038 | 49 | 115,268 | 36,301 | 31.5 | 2.3 | 2.9 | 60.6 | 28,002 |
| cigo-version | 1038 | 15 | 115,268 | 34,979 | 30.3 | 1.1 | 0.9 | 14.8 | 31,286 |

### What was made and copied (per 1,000 instructions)

A `mov` of a string, list, map or function, and a `loadk` of a string, each copy a counted value: one reference-count increment, and a decrement when the register is overwritten.

| program | lists | maps | closures | interpolated strings | `mov` of a counted value | `loadk` of a string | `mov` of a counted value (% of `mov`) |
|---|---:|---:|---:|---:|---:|---:|---:|
| cigo-audit-one | 44.52 | 15.41 | 14.57 | 0.04 | 112.02 | 242.00 | 91.6 |
| cigo-guard-line | 33.75 | 13.59 | 8.81 | 0.01 | 116.59 | 229.70 | 98.2 |
| cigo-help | 69.11 | 24.67 | 17.81 | 0.08 | 31.93 | 390.09 | 97.2 |
| cigo-hook-benign | 33.57 | 13.55 | 8.79 | 0.00 | 116.83 | 228.94 | 98.0 |
| cigo-hook-curl-sh | 32.25 | 13.08 | 8.49 | 0.00 | 114.90 | 220.11 | 97.2 |
| cigo-list | 36.73 | 11.76 | 15.21 | 2.66 | 128.05 | 217.70 | 88.3 |
| cigo-version | 69.59 | 24.83 | 17.92 | 0.05 | 30.33 | 390.82 | 97.1 |

### The builtins and methods called most

- **cigo-audit-one:** builtins: `type_of` 384, `str` 373, `text.find_all` 158, `path.join` 157, `fs.is_file` 121. Methods: `.get()` 460, `.has()` 397, `.replace()` 330, `.map()` 282, `.push()` 164.
- **cigo-guard-line:** builtins: `type_of` 3,147, `str` 64, `env.get` 56, `path.join` 38, `text.matches` 38. Methods: `.push()` 1,281, `.sort()` 82, `.keys()` 79, `.has()` 38, `.upper()` 32.
- **cigo-help:** builtins: `path.join` 38, `env.get` 18, `cig.os` 4, `path.normalize` 2, `fs.home` 2. Methods: `.merge()` 9, `.get()` 2, `.filter()` 2, `.join()` 2, `.slice()` 1.
- **cigo-hook-benign:** builtins: `type_of` 3,149, `str` 69, `env.get` 59, `text.matches` 43, `path.join` 38. Methods: `.push()` 1,282, `.sort()` 82, `.keys()` 79, `.has()` 41, `.get()` 37.
- **cigo-hook-curl-sh:** builtins: `type_of` 3,149, `str` 73, `env.get` 59, `text.matches` 44, `path.join` 41. Methods: `.push()` 1,284, `.slice()` 93, `.sort()` 82, `.keys()` 79, `.get()` 42.
- **cigo-list:** builtins: `str` 962, `type_of` 632, `text.find_all` 156, `path.base` 106, `text.matches` 106. Methods: `.get()` 1,112, `.has()` 786, `.map()` 636, `.replace()` 364, `.len()` 264.
- **cigo-version:** builtins: `path.join` 38, `env.get` 19, `cig.os` 4, `path.normalize` 2, `fs.home` 2. Methods: `.merge()` 9, `.get()` 2, `.filter()` 2, `.slice()` 1, `.map()` 1.

## Start-up programs

### Instructions executed, by group (percent of the program's instructions)

| program | instructions | sites | moves and constants | arithmetic | comparison and logic | control | step marks | data | calls and returns | closures and cells | everything else |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| hello | 6 | 6 | 50.0 | 0.0 | 0.0 | 0.0 | 16.7 | 0.0 | 16.7 | 0.0 | 16.7 |
| lines-500 | 3,287 | 1572 | 22.8 | 12.7 | 2.6 | 19.2 | 25.2 | 9.3 | 6.9 | 1.2 | 0.1 |
| startup-0 | 6 | 6 | 50.0 | 0.0 | 0.0 | 0.0 | 16.7 | 0.0 | 16.7 | 0.0 | 16.7 |
| startup-1 | 13 | 13 | 38.5 | 7.7 | 0.0 | 0.0 | 15.4 | 15.4 | 15.4 | 0.0 | 7.7 |
| startup-10 | 71 | 71 | 46.5 | 1.4 | 0.0 | 0.0 | 15.5 | 19.7 | 15.5 | 0.0 | 1.4 |

### The script's top level (function 0)

What runs once a process before anything else can: the statements outside every function, which declare the script's functions and build its tables.

| program | instructions | in the top level | of the run (%) | its busiest instructions |
|---|---:|---:|---:|---:|
| hello | 6 | 6 | 100.0 | `steps` 1, `loadk` 1, `mov` 1, `exhale` 1, `loadnull` 1, `ret` 1 |
| lines-500 | 3,287 | 382 | 11.6 | `mov` 84, `steps` 44, `loadk` 43, `loadint` 42, `insertmap` 42, `closure` 41 |
| startup-0 | 6 | 6 | 100.0 | `steps` 1, `loadk` 1, `mov` 1, `exhale` 1, `loadnull` 1, `ret` 1 |
| startup-1 | 13 | 13 | 100.0 | `steps` 2, `mov` 2, `loadk` 2, `loadint` 1, `neg` 1, `callb` 1 |
| startup-10 | 71 | 71 | 100.0 | `loadk` 18, `steps` 11, `mov` 11, `insertmap` 11, `callb` 9, `loadint` 4 |

### Arithmetic: the operand types seen (percent of the group's executions)

| program | executed | int, int | float, float | int and float | str, str | other |
|---|---:|---:|---:|---:|---:|---:|
| hello | 0 | - | - | - | - | - |
| lines-500 | 417 | 100.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| startup-0 | 0 | - | - | - | - | - |
| startup-1 | 0 | - | - | - | - | - |
| startup-10 | 0 | - | - | - | - | - |

### Comparisons: the operand types seen (percent of the group's executions)

| program | executed | int, int | float, float | int and float | str, str | other |
|---|---:|---:|---:|---:|---:|---:|
| hello | 0 | - | - | - | - | - |
| lines-500 | 84 | 100.0 | 0.0 | 0.0 | 0.0 | 0.0 |
| startup-0 | 0 | - | - | - | - | - |
| startup-1 | 0 | - | - | - | - | - |
| startup-10 | 0 | - | - | - | - | - |

### How settled a site's types are

Arithmetic and comparison sites. "One kind" is a site that saw one pair of operand types for the whole run.

| program | sites | sites with one kind | executions | at a site with one kind (%) |
|---|---:|---:|---:|---:|
| hello | 0 | 0 | 0 | - |
| lines-500 | 164 | 164 | 501 | 100.0 |
| startup-0 | 0 | 0 | 0 | - |
| startup-1 | 0 | 0 | 0 | - |
| startup-10 | 0 | 0 | 0 | - |

### Field reads and writes (`getfield`, `setfield`)

A shape is a record's keys in order (a map of up to 16 keys); a larger map is a dictionary. "One position" is a site where the key sat at one index for the whole run, which is what the VM's cache cell holds today.

| program | executed | sites | sites with one shape | one shape (%) | two (%) | 3 to 8 (%) | more than 8 (%) | one position (%) | on a dictionary (%) | not on a map (%) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| hello | 0 | 0 | 0 | - | - | - | - | - | - | - |
| lines-500 | 0 | 0 | 0 | - | - | - | - | - | - | - |
| startup-0 | 0 | 0 | 0 | - | - | - | - | - | - | - |
| startup-1 | 0 | 0 | 0 | - | - | - | - | - | - | - |
| startup-10 | 0 | 0 | 0 | - | - | - | - | - | - | - |

### Indexing (`getindex`, `setindex`): what was indexed by what (percent of executions)

| program | executed | at a site with one kind (%) | map[str] |
|---|---:|---:|---:|
| hello | 0 | - | - |
| lines-500 | 103 | 100.0 | 100.0 |
| startup-0 | 0 | - | - |
| startup-1 | 0 | - | - |
| startup-10 | 0 | - | - |

### Calls

`call` and `tailcall` call a value; `callb` calls a builtin the lowering resolved; `callm` calls a method on a value.

| program | `call` | sites | sites with one callee | at a site with one callee (%) | to a function of the script (%) | to a builtin value (%) | `callb` | `callm` | `callm` at a site with one receiver type (%) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| hello | 0 | 0 | 0 | - | - | - | 0 | 0 | - |
| lines-500 | 41 | 41 | 41 | 100.0 | 100.0 | 0.0 | 41 | 102 | 100.0 |
| startup-0 | 0 | 0 | 0 | - | - | - | 0 | 0 | - |
| startup-1 | 0 | 0 | 0 | - | - | - | 1 | 0 | - |
| startup-10 | 0 | 0 | 0 | - | - | - | 9 | 1 | 100.0 |

### Functions of the script: rows of argument types

For each function called through `call`: how many distinct rows of argument types it was called with (up to 16 are kept).

| program | functions called | called with one row | calls | calls to a function with one row (%) | most rows at one function |
|---|---:|---:|---:|---:|---:|
| hello | 0 | 0 | 0 | - | 0 |
| lines-500 | 41 | 41 | 41 | 100.0 | 1 |
| startup-0 | 0 | 0 | 0 | - | 0 |
| startup-1 | 0 | 0 | 0 | - | 0 |
| startup-10 | 0 | 0 | 0 | - | 0 |

### Where the work is

The module is the script as lowered: every function it holds and every instruction in them. A site executed is an instruction that ran at least once.

| program | functions in the module | functions entered | instructions in the module | sites executed | of the module (%) | executions a site | ten busiest sites (%) | busiest tenth of the sites (%) | sites that make 90% of the instructions |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| hello | 1 | 1 | 6 | 6 | 100.0 | 1.0 | 100.0 | 16.7 | 6 |
| lines-500 | 42 | 42 | 1,662 | 1,572 | 94.6 | 2.1 | 2.4 | 30.8 | 1,244 |
| startup-0 | 1 | 1 | 6 | 6 | 100.0 | 1.0 | 100.0 | 16.7 | 6 |
| startup-1 | 1 | 1 | 13 | 13 | 100.0 | 1.0 | 76.9 | 7.7 | 12 |
| startup-10 | 1 | 1 | 71 | 71 | 100.0 | 1.0 | 14.1 | 9.9 | 64 |

### What was made and copied (per 1,000 instructions)

A `mov` of a string, list, map or function, and a `loadk` of a string, each copy a counted value: one reference-count increment, and a decrement when the register is overwritten.

| program | lists | maps | closures | interpolated strings | `mov` of a counted value | `loadk` of a string | `mov` of a counted value (% of `mov`) |
|---|---:|---:|---:|---:|---:|---:|---:|
| hello | 0.00 | 0.00 | 0.00 | 0.00 | 166.67 | 166.67 | 100.0 |
| lines-500 | 4.26 | 4.26 | 12.47 | 40.46 | 85.18 | 57.80 | 61.5 |
| startup-0 | 0.00 | 0.00 | 0.00 | 0.00 | 166.67 | 166.67 | 100.0 |
| startup-1 | 0.00 | 76.92 | 0.00 | 0.00 | 76.92 | 153.85 | 50.0 |
| startup-10 | 14.08 | 28.17 | 0.00 | 0.00 | 126.76 | 253.52 | 81.8 |

### The builtins and methods called most

- **hello:** builtins: none. Methods: none.
- **lines-500:** builtins: `range` 41. Methods: `.push()` 69, `.len()` 19, `.join()` 14.
- **startup-0:** builtins: none. Methods: none.
- **startup-1:** builtins: `math.abs` 1. Methods: none.
- **startup-10:** builtins: `math.abs` 1, `str` 1, `json.stringify` 1, `path.base` 1, `time.format_ms` 1. Methods: `.split()` 1.

