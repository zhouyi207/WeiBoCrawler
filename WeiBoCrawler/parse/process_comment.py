from typing import Tuple
import httpx
from pydantic import BaseModel
import pandas as pd
from ..util import process_base_documents, process_base_document

class CommmentResponseInfo(BaseModel):
    max_id: str
    total_number: int
    data_number: int



def process_comment_resp(resp: httpx.Response) -> Tuple[CommmentResponseInfo, list]:
    """处理评论数据

    这里有三种方式判断 resp 是否正常：
    1. 正常响应头中会有 content-encoding:gzip, 而不正常的响应头中相应位置为 content-length: 117(或者其他)
    2. 正常响应中会有 filter_group 字段, 不正常响应中没有该字段,
    3. 无论正常还是非正常响应中都有 data 字段, 正常响应 data 字段内容为 [dict], 非正常响应 data 字段内容为 []

    目前使用第三种方法.
    
    Args:
        resp (httpx.Response): 接受到的响应.

    Returns:
        Tuple[dict, list]: 前面是 请求的信息(后面要用到), 后面是数据
    """
    data = resp.json()
    max_id = data.get("max_id", "")
    total_number = data.get("total_number", 0)
    data_number = len(data.get("data", []))

    data_list = data["data"]

    transform_dict = {
            "mid": "mid",
            "uid": ["user", "idstr"],
    }
    
    [data.update(process_base_document(data, transform_dict)) for data in data_list]

    resp_info = CommmentResponseInfo(max_id=str(max_id), total_number=int(total_number), data_number=data_number)
    return resp_info, data_list





def process_comment_documents(documents: list[dict]) -> pd.DataFrame:
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
        "f_mid": "f_mid",
        "f_uid": "f_uid",
        "mid": "mid",
        "uid": ["user", "id"],
        "个人昵称": ["user", "screen_name"],
        "用户性别": ["user", "gender"],
        "用户定位": ["user", "location"],
        "用户粉丝": ["user", "followers_count"],
        "用户累计评论数": ["user", "status_total_counter", "comment_cnt"],
        "用户累计转发数": ["user", "status_total_counter", "repost_cnt"],
        "用户累计点赞数": ["user", "status_total_counter", "like_cnt"],
        "用户累计评转赞": ["user", "status_total_counter", "total_cnt"],
        "发布时间": "created_at",
        "原生内容": "text",
        "展示内容": "text_raw",
        "评论数量": "total_number",
        "点赞数量": "like_counts",
    }
    df = process_base_documents(documents, transform_dict)
    return df