import pandas as pd
from tinydb.table import Table
from ..util import process_base_table

def process_list_table(table: Table) -> pd.DataFrame:
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
    transform_dict = {
            "mid": "mid",
            "uid": "uid",
            "mblogid": "mblogid",
            "个人昵称": "personal_name",
            "个人主页": "personal_href",
            "微博链接": "weibo_href",
            "发布时间": "publish_time",
            "内容来自": "content_from",
            "全部内容": "content_all",
            "转发数量": "retweet_num",
            "评论数量": "comment_num",
            "点赞数量": "star_num",
        }
    df = process_base_table(table, transform_dict)
    return df