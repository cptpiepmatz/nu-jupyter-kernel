import argparse
import sys
from typing import Any

from jupyter_client import KernelManager

type Cells = list[str]


def run_cells(kernel_name: str, cells: Cells, timeout: float = 30.0) -> int:
    km = KernelManager(kernel_name=kernel_name)
    km.start_kernel()
    kc = km.client()
    kc.start_channels()

    try:
        kc.wait_for_ready(timeout=10)
    except Exception as exc:
        print(f"Kernel did not become ready: {exc}", file=sys.stderr)
        kc.stop_channels()
        km.shutdown_kernel(now=True)
        return 1

    had_error = False

    try:
        for index, cell in enumerate(cells, start=1):
            print(f"== Cell {index} ==")
            print(cell)
            print()

            msg_id: str = kc.execute(cell)

            while True:
                try:
                    msg: dict[str, Any] = kc.get_iopub_msg(timeout=timeout)
                except Exception as exc:
                    print(f"[control] Timeout while waiting for output: {exc}", file=sys.stderr)
                    had_error = True
                    break

                if msg["parent_header"].get("msg_id") != msg_id:
                    continue

                msg_type: str = msg["header"]["msg_type"]
                content: dict[str, Any] = msg["content"]

                match msg_type:
                    case "status":
                        if content.get("execution_state") == "idle":
                            break

                    case "stream":
                        text: str = content.get("text", "")
                        name: str | None = content.get("name")

                        if name == "stderr":
                            print(f"[stderr] {text}", end="", file=sys.stderr)
                        else:
                            print(f"[stdout] {text}", end="", file=sys.stdout)

                    case "execute_result" | "display_data":
                        data: dict[str, Any] = content.get("data", {})
                        text = data.get("text/plain")

                        if text is None:
                            print(f"[{msg_type}] {data}")
                        elif msg_type == "execute_result":
                            print(f"[result] {text}")
                        else:
                            print(f"[display] {text}")

                    case "error":
                        had_error = True
                        print("[error]", file=sys.stderr)
                        for line in content.get("traceback", []):
                            print(line, file=sys.stderr)

                    case _:
                        continue

            print()

    finally:
        kc.stop_channels()
        km.shutdown_kernel(now=False)

    return 1 if had_error else 0


def build_parser() -> argparse.ArgumentParser:
    parser: argparse.ArgumentParser = argparse.ArgumentParser(
        description="Send a list of cells to a Jupyter kernel and show labeled outputs."
    )
    parser.add_argument(
        "--kernel",
        "-k",
        default="nu",
        help="Kernel name (as in `jupyter kernelspec list`). Default: nu",
    )
    parser.add_argument(
        "--timeout",
        "-t",
        type=float,
        default=30.0,
        help="Timeout in seconds while waiting for output per cell. Default: 30",
    )
    parser.add_argument(
        "cells",
        nargs="+",
        help="Code cells to execute, each as a separate argument",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    if argv is None:
        argv = sys.argv[1:]

    parser = build_parser()
    args = parser.parse_args(argv)

    return run_cells(
        kernel_name=args.kernel,
        cells=args.cells,
        timeout=args.timeout,
    )


if __name__ == "__main__":
    raise SystemExit(main())
