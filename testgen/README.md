# testgen

`testgen.py` is a Python script that scrapes the Lichess puzzle database and
runs eccat on each puzzle to test if it finds the right move.

The script automatically downloads the puzzle database from Lichess. It also
rebuilds eccat each time it is run.

## Prerequisites

-   `python3`
-   `wget` (to download the puzzle database)
-   `zstd` (to decompress the database)

## Usage

```sh
# run eccat on the puzzles
python3 testgen.py

# run eccat on the puzzles using pext optimizations
python3 testgen.py pext

# gnerate a list of 2000 fen strings
# and write them to a file called "fens.txt"
# in the same directory as the script
# (default is 1000 if no number is given)
python3 testgen.py genfens 2000
```
