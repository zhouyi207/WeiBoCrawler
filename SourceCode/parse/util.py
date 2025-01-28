import re
from datetime import datetime, timedelta
from ..util import custom_validate_call


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