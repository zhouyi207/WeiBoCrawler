import re
from datetime import datetime, timedelta
from typing import Callable, Optional

import toml
from pydantic import BaseModel, validate_call, field_validator
from rich.progress import (
    BarColumn,
    MofNCompleteColumn,
    Progress,
    TextColumn,
    TimeElapsedColumn,
)

import pandas as pd
from pathlib import Path
from tinydb.table import Table

import logging

module_path = Path(__file__).parent.parent


class Database_Config(BaseModel):
    list: str
    body: str
    comment1: str
    comment2: str

    @field_validator('list', 'body', 'comment1', 'comment2')
    def add_module_path(cls, value):
        return str(module_path / value)


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


database_config_path = module_path / "./config.toml"
request_params_path = module_path / "./request/request.toml"
database_config = Database_Config.model_validate(toml.load(database_config_path)["database"])
request_params = RequestParams.model_validate(toml.load(request_params_path))


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



def drop_table_duplicates(table: Table) -> None:
    """表格去重
    这里暂时使用最简单的列表去重法, 后续可以考虑使用 hash 去重等方法优化..

    Args:
        table (Table): 需要去重的表
    """
    unique_document = []
    for document in table.all():
        if document not in unique_document:
            unique_document.append(document)
    
    table.truncate()
    table.insert_multiple(unique_document)


def process_base_table(table: Table, transform_dict: dict) -> pd.DataFrame:
    """将表处理成 dataframe 的形式
    
    transform_dict = {
            "转发数量": "retweet_num",
            "评论数量": "comment_num",
            "点赞数量": "star_num",
            ...
        }

    Args:
        table (Table): 需要处理的表
        transform_dict (dict): 转换字典, key 是转化后的字段, value 是原始字段

    Returns:
        pd.DataFrame: (去重)处理后得到的表格
    """
    items = []
    for document in table.all():
        item = {}
        try:
            for key, value in transform_dict.items():
                if isinstance(value, str):
                    final_value = document.get(value, None)

                elif isinstance(value, list):
                    final_value = document
                    for v in value:
                        if final_value is None:
                            break
                        final_value = final_value.get(v, None)

                item[key] = final_value
        except Exception as e:
            print(e)
            print(document)
            
        items.append(item)
    df = pd.DataFrame(items)
    df.drop_duplicates(inplace=True)
    return df


# 配置日志
logging.basicConfig(
    filename=module_path / "./app.log",
    level=logging.INFO, 
    format='%(asctime)s - %(levelname)s - %(name)s - %(message)s',
    encoding="utf-8",
)


def log_function_params(logger: logging.Logger):
    """记录函数的参数和返回值

    Args:
        func (Callable): 需要装饰的函数
           
    Returns:
        Callable: 装饰后的函数
    """
    def log_function_params_(func:Callable) -> Callable:
        def wrapper(*args, **kwargs):
            # 记录函数名和参数
            args_repr = [repr(a) for a in args]
            kwargs_repr = [f"{k}={v!r}" for k, v in kwargs.items()]
            signature = ", ".join(args_repr + kwargs_repr)
            logger.info(f"Calling Function {func.__name__}({signature})")
            
            # 调用原函数
            result = func(*args, **kwargs)
            
            # 记录返回值
            logger.info(f"Function {func.__name__} returned {result!r}")
            return result
        return wrapper
    return log_function_params_