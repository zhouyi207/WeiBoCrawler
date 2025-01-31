import pandas as pd
from ..util import process_base_document, process_base_documents

def process_body_resp(resp):
    """处理详细页数据

    这里一般都会收到正常的响应，所以只需要处理数据即可.
    Args:
        resp (httpx.Response): 接受到的响应.

    Returns:
        list[dict]: 响应的数据, 这里使用 list 包装一下(对齐其他的process请求).
    """
    data = resp.json()
    transform_dict = {
            "mid": "mid",
            "uid": ["user", "idstr"],
    }
    data.update(process_base_document(data, transform_dict))
    return [data]


def process_body_documents(documents: list[dict]) -> pd.DataFrame:
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
    transform_dict = {
            "mid": "mid",
            "uid": ["user", "idstr"],
            "mblogid": "mblogid",
            "个人昵称": ["user", "screen_name"],

            "用户性别": ["longText", "user", "gender"],

            "用户定位": ["longText","user", "location"],
            "用户粉丝": ["longText","user", "followers_count"],
            "用户累计评论数": ["user", "status_total_counter", "comment_cnt"],
            "用户累计转发数": ["user", "status_total_counter", "repost_cnt"],
            "用户累计点赞数": ["user", "status_total_counter", "like_cnt"],
            "用户累计评转赞": ["user", "status_total_counter", "total_cnt"],
            "发布时间": "created_at",
            "原生内容": "text",
            "展示内容": "text_raw",
            
            "转发数量": "reposts_count",
            "评论数量": "comments_count",
            "点赞数量": "attitudes_count",
        }
    df = process_base_documents(documents, transform_dict)
    return df