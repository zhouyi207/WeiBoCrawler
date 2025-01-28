from typing import Tuple
import httpx
from pydantic import BaseModel


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
    resp_info = CommmentResponseInfo(max_id=str(data["max_id"]), total_number=int(data["total_number"]), data_number=len(data["data"]))
    return resp_info, data["data"]