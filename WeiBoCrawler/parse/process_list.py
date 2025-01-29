import pandas as pd
from ..util import drop_table_duplicates, Table

def process_list_table(table: Table) -> pd.DataFrame:
    df = drop_table_duplicates(table)
    need_info = {
        "mid" : "mid",
        "uid" : "uid",
        "mblogid" : "mblogid",
        "personal_name" : "个人昵称",
        "personal_href" : "个人主页",
        "weibo_href" : "微博链接",
        "publish_time" : "发布时间",
        "content_from" : "内容来自",
        "content_all" : "全部内容",
        "retweet_num" : "转发数量",
        "comment_num" : "评论数量",
        "star_num" : "点赞数量",
    }
    df = df[list(need_info.keys())]
    df.rename(columns=need_info, inplace=True)
    return df