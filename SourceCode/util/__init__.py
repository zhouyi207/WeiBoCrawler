import re
from datetime import datetime, timedelta
from typing import Callable, Optional

import toml
from pydantic import BaseModel, validate_call
from rich.progress import (
    BarColumn,
    MofNCompleteColumn,
    Progress,
    TextColumn,
    TimeElapsedColumn,
)


class Database_Config(BaseModel):
    list: str
    body: str
    comment1: str
    comment2: str


class RequestParams(BaseModel):
    """这个类主要用来保存一些请求参数的东西

    Attributes:
        body_headers (dict): 微博主页的请求头
        comment1_buildComments_headers (dict): 评论区buildComments的请求头
        comment1_rum_headers (dict): 评论区rum的请求头
        cookies (dict): 微博的cookies
        update_time (datetime): 更新时间    
    """
    list_headers: dict
    body_headers: dict
    comment1_buildComments_headers: dict
    comment1_rum_headers: dict
    comment2_buildComments_headers: dict
    comment2_rum_headers: dict
    cookies: dict
    update_time: datetime = Optional[datetime]


database_config = Database_Config.model_validate(toml.load("./SourceCode/config.toml")["database"])
request_params = RequestParams.model_validate(toml.load("./SourceCode/request/request.toml"))


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


def custom_validate_call(func: Callable) -> Callable:
    return validate_call(func, config={"arbitrary_types_allowed": True}, validate_return=True)



@custom_validate_call
def process_time_str(time_str:str) -> datetime:
    """这段代码是用来解析微博的时间字段的
         1. 处理 年、月、日、时、分
         2. 处理 分钟前，小时前，这里不处理秒前

    Args:
        time_str (str): 微博时间字段

    Returns:
        datatime: 返回时间字段
    """
    datetime_now = datetime.now()

    if "年" in time_str:
        year = re.search(r"(\d{4})年", time_str).group(1)
    else:
        year = datetime_now.year
    if "月" in time_str:
        month = re.search(r"(\d{1,2})月", time_str).group(1)
    else:
        month = datetime_now.month
    if "日" in time_str:
        day = re.search(r"(\d{1,2})日", time_str).group(1)
    else:
        day = datetime_now.day
    if ":" in time_str:
        hour = re.search(r"(\d{1,2}):", time_str).group(1)
        minute = re.search(r":(\d{1,2})", time_str).group(1)
    else:
        hour = datetime_now.hour
        minute = datetime_now.minute

    datetime_now = datetime(int(year), int(month), int(day), int(hour), int(minute))

    if "分钟前" in time_str:
        minute_before = re.search(r"(\d+)分钟前", time_str).group(1)
        datetime_now = datetime_now - timedelta(minutes=int(minute_before))
    if "小时前" in time_str:
        hour_before = re.search(r"(\d+)小时前", time_str).group(1)
        datetime_now = datetime_now - timedelta(hours=int(hour_before))

    return datetime_now








