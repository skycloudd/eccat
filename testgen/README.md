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
python3 testgen.py
# or
./testgen.py
```
