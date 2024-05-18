#!/usr/bin/env python3

import os
import pathlib
import csv
import chess
import chess.engine


class Puzzle:
    def __init__(self, puzzle_id: str, fen: str, rating: int, moves: list[str]):
        self.puzzle_id = puzzle_id
        self.fen = fen
        self.rating = rating
        self.moves = moves


def download_puzzle_db(download_path: pathlib.Path):
    print(f"Downloading puzzle database to {download_path}")

    if download_path.exists():
        print("File already exists, skipping download")
        print("If you want to redownload, delete the file and run the script again")
        return

    url = "https://database.lichess.org/lichess_db_puzzle.csv.zst"

    os.system(f"wget {url} -O {download_path}")


def generate_list(puzzles_path: pathlib.Path, n: int) -> list[Puzzle]:
    puzzles: list[Puzzle] = []

    with open(puzzles_path, "r") as f:
        reader = csv.reader(f)

        next(reader)

        for row in reader:
            (puzzle_id, fen, moves, rating, _, _, _, _, _, _) = row

            moves = moves.split(" ")

            puzzle = Puzzle(puzzle_id, fen, int(rating), moves)

            puzzles.append(puzzle)

            if len(puzzles) >= n:
                break

    puzzles = sorted(puzzles, key=lambda x: x.rating)

    return puzzles


def download_puzzles() -> pathlib.Path:
    download_path = (
        pathlib.Path(__file__).resolve().parent / "lichess_db_puzzle.csv.zst"
    )
    puzzles_path = download_path.parent / "lichess_db_puzzle.csv"

    if puzzles_path.exists():
        print(f"`{puzzles_path}` already exists, skipping download")
        print("If you want to redownload, delete the file and run the script again")
        return puzzles_path

    download_puzzle_db(download_path)

    os.system(f"zstd --decompress {download_path}")

    os.remove(download_path)

    return puzzles_path


def main(engine: chess.engine.SimpleEngine):
    puzzles_path = download_puzzles()

    print(f"Generating list of puzzles from {puzzles_path}")

    limit = chess.engine.Limit(time=1.0)

    engine.configure({"Hash": 64})

    print(f"Searching with {limit}")

    print(f'Running engine `{engine.id["name"]}`')

    puzzles = generate_list(puzzles_path, 100)

    for i, puzzle in enumerate(puzzles):
        board = chess.Board(puzzle.fen)

        board.push_uci(puzzle.moves[0])

        print(
            f"---\nPuzzle {i + 1}\tid: {puzzle.puzzle_id}, r: {puzzle.rating}, fen: {board.fen()}"
        )

        info = engine.analyse(board, limit)

        score = info.get("score")
        best_move = (info.get("pv") or [chess.Move.from_uci("0000")])[0]
        depth = info.get("depth")
        seldepth = info.get("seldepth")
        nodes = info.get("nodes")
        nps = info.get("nps")

        nps_str = ""

        if nps is not None:
            if nps > 1_000:
                nps_str = f"{nps / 1_000_000:.1f}M"
            else:
                nps_str = f"{nps}"

        if score is not None:
            score = score.relative

        uci_best_move = best_move.uci()
        puzzle_best_move = puzzle.moves[1]

        if uci_best_move == puzzle_best_move:
            print(f"Correct move:  \t{uci_best_move}")
            print(f"Depth:         \t{depth}/{seldepth}")
            print(f"Nodes:         \t{nodes}")
            print(f"NPS:           \t{nps_str}")
            print(f"Relative score:\t{score}")
        else:
            print(f"Wrong move:    \tfound {best_move}, best: {puzzle_best_move}")
            print(f"Depth:         \t{depth}/{seldepth}")
            print(f"Nodes:         \t{nodes}")
            print(f"NPS:           \t{nps_str}")
            print(f"Relative score:\t{score}")
            return


if __name__ == "__main__":
    engine_cmd = "../target/full/eccat"
    os.system("cargo build --profile full")

    engine = chess.engine.SimpleEngine.popen_uci(engine_cmd)

    try:
        main(engine)
    except Exception as e:
        print(e)
    finally:
        engine.quit()
        print("Engine quit")
