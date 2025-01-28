from pydantic import validate_call
from typing import Callable
from rich.progress import Progress, BarColumn, TimeElapsedColumn, TextColumn, MofNCompleteColumn


def custom_validate_call(func: Callable) -> Callable:
    return validate_call(func, config={"arbitrary_types_allowed": True}, validate_return=True)


class CustomProgress:
    """自定义进度条

    Attributes:
        progress (Progress): 进度条
    """
    def __init__(self):
        self.progress = Progress(
            BarColumn(),
            MofNCompleteColumn(),
            TimeElapsedColumn(),
            TextColumn("[progress.description]{task.description}", justify="left"),
        )

    def __enter__(self):
        self.progress.start()
        return self.progress

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.progress.stop()