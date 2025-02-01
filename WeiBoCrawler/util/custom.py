from pydantic import BaseModel
from rich.progress import (
    BarColumn,
    MofNCompleteColumn,
    Progress,
    TextColumn,
    TimeElapsedColumn,
)



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



class RequestHeaders(BaseModel):
    """这个类主要用来保存一些请求参数的东西

    Attributes:
        body_headers (dict): 微博主页的请求头
        comment1_buildComments_headers (dict): 评论区buildComments的请求头
        comment1_rum_headers (dict): 评论区rum的请求头
        ....
    """
    list_headers: dict
    body_headers: dict
    comment1_buildComments_headers: dict
    comment1_rum_headers: dict
    comment2_buildComments_headers: dict
    comment2_rum_headers: dict
    login_signin_headers:dict
    login_qrcode_headers:dict
    login_final_headers:dict