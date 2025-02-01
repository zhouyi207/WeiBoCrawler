import re
from datetime import datetime, timedelta
import pandas as pd


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



def drop_documents_duplicates(documents: list[dict]) -> None:
    """dict 列表去重
    这里暂时使用最简单的列表去重法, 后续可以考虑使用 hash 去重等方法优化..

    Args:
        list[dict]: 去重后的表
    """
    unique_document = []
    for document in documents:
        if document not in unique_document:
            unique_document.append(document)
    
    return unique_document


def process_base_document(document: dict, transform_dict: dict) -> dict:
    """将 document 处理成字典的形式

    transform_dict = {
            "转发数量": "retweet_num",
            "评论数量": "comment_num",
            "点赞数量": "star_num
          ...
        }

    Args:
        document (dict): 文档
        transform_dict (dict): 转换字典, key 是转化后的字段, value 是原始字段

    Returns:
        dict: 处理后的字典
    """
    item = {}

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
    return item


def process_base_documents(documents: list[dict], transform_dict: dict) -> pd.DataFrame:
    """将 documents 处理成 dataframe 的形式
    
    transform_dict = {
            "转发数量": "retweet_num",
            "评论数量": "comment_num",
            "点赞数量": "star_num",
            ...
        }

    Args:
        documents (list[dict]): 文档列表
        transform_dict (dict): 转换字典, key 是转化后的字段, value 是原始字段

    Returns:
        pd.DataFrame: (去重)处理后得到的表格
    """
    items = [process_base_document(document, transform_dict) for document in documents]
    df = pd.DataFrame(items)
    df.drop_duplicates(inplace=True)
    return df